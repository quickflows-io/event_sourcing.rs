use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use thiserror::Error;
use uuid::Uuid;

use esrs::postgres::PgStore;
use esrs::{Aggregate, AggregateManager, AggregateState, Policy, StoreEvent};

#[derive(Clone)]
pub struct LoggingAggregate {
    event_store: PgStore<Self>,
}

impl LoggingAggregate {
    pub async fn new(pool: &Pool<Postgres>) -> Result<Self, LoggingError> {
        let event_store: PgStore<LoggingAggregate> = PgStore::new(pool.clone()).setup().await?;

        let this: Self = Self {
            // This clone is cheap being that the internal fields of the store are behind an Arc
            event_store: event_store.clone(),
        };

        event_store.set_policies(vec![Box::new(LoggingPolicy::new(this.clone()))]);
        Ok(this)
    }
}

impl Aggregate for LoggingAggregate {
    type State = u64;
    type Command = LoggingCommand;
    type Event = LoggingEvent;
    type Error = LoggingError;

    fn handle_command(_state: &Self::State, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            Self::Command::TryLog(msg) => Ok(vec![Self::Event::Received(msg)]),
            Self::Command::Succeed => Ok(vec![Self::Event::Succeeded]),
            Self::Command::Fail => Ok(vec![Self::Event::Failed]),
        }
    }

    fn apply_event(state: Self::State, event: Self::Event) -> Self::State {
        match event {
            LoggingEvent::Received(_) => {
                // Do nothing, since the logging policy is what carries out the side effect
                // NOTE that, in this case, the Self::State we return isn't valid any more,
                // since the policy runs another command on this aggregate, causing a state
                // update which isn't applied to the state returned here
            }
            LoggingEvent::Succeeded => {
                // No-op, we simply persist the event in the event log
                // This is where some success notification might be sent to another service,
                // or some other success handling would go, in a more complex system
            }
            LoggingEvent::Failed => {
                // No-op, we simply persist the event in the event log
                // This is where some cleanup action would go in a more complex system
            }
        }
        state + 1
    }
}

impl AggregateManager for LoggingAggregate {
    type EventStore = PgStore<Self>;

    fn name() -> &'static str {
        "message"
    }

    fn event_store(&self) -> &Self::EventStore {
        &self.event_store
    }
}

// A very simply policy, which tries to log an event, and creates another command after it finishes
// which indicates success or failure to log
#[derive(Clone)]
struct LoggingPolicy {
    aggregate: LoggingAggregate,
}

impl LoggingPolicy {
    pub fn new(aggregate: LoggingAggregate) -> Self {
        Self { aggregate }
    }
}

#[async_trait]
impl Policy<LoggingAggregate> for LoggingPolicy {
    async fn handle_event(&self, event: &StoreEvent<LoggingEvent>) -> Result<(), LoggingError> {
        let aggregate_id: Uuid = event.aggregate_id;

        let aggregate_state: AggregateState<u64> = self
            .aggregate
            .load(aggregate_id)
            .await
            .unwrap_or_else(|| AggregateState::new(aggregate_id)); // This should never happen

        if let LoggingEvent::Received(msg) = event.payload() {
            if msg.contains("fail_policy") {
                self.aggregate
                    .handle_command(aggregate_state, LoggingCommand::Fail)
                    .await?;
                return Err(LoggingError::Domain(msg.clone()));
            }
            println!("Logged via policy from {}: {}", aggregate_id, msg);
            self.aggregate
                .handle_command(aggregate_state, LoggingCommand::Succeed)
                .await?;
        }

        Ok(())
    }
}

// A simple error enum for event processing errors
#[derive(Debug, Error)]
pub enum LoggingError {
    #[error("Err {0}")]
    Domain(String),

    #[error(transparent)]
    Json(#[from] esrs::error::JsonError),

    #[error(transparent)]
    Sql(#[from] esrs::error::SqlxError),
}

// The events to be processed. On receipt of a new log message, the aggregate stores a Received event.
// If the message contained within can be logged, it is, and then a Succeeded event is produced, otherwise
// a Failed event is produced.
#[derive(Serialize, Deserialize, Debug)]
pub enum LoggingEvent {
    Received(String),
    Succeeded,
    Failed,
}

// The aggregate receives commands to log a message
pub enum LoggingCommand {
    TryLog(String),
    Succeed,
    Fail,
}