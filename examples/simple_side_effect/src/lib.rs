use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use thiserror::Error;

use esrs::{Aggregate, EventHandler, StoreEvent, TransactionalEventHandler};

pub struct LoggingAggregate;

impl Aggregate for LoggingAggregate {
    const NAME: &'static str = "message";

    type State = u64;
    type Command = LoggingCommand;
    type Event = LoggingEvent;
    type Error = LoggingError;

    fn handle_command(_state: &Self::State, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            Self::Command::Log(msg) => Ok(vec![Self::Event::Logged(msg)]),
        }
    }

    fn apply_event(state: Self::State, _: Self::Event) -> Self::State {
        // This aggregate state just counts the number of applied - equivalent to the number in the event store
        state + 1
    }
}

// Here's a simple projection that just carries out a side effect, and can fail,
// causing the event not to be written to the event store. In this case the
// failure is due to a simple log message filter rule, but you can imagine a
// side effect which interacts with some 3rd party service in a failable way instead
#[derive(Clone)]
pub struct LoggingTransactionalEventHandler;

#[async_trait]
impl TransactionalEventHandler<LoggingAggregate, PgConnection> for LoggingTransactionalEventHandler {
    async fn handle(&self, event: &StoreEvent<LoggingEvent>, _: &mut PgConnection) -> Result<(), LoggingError> {
        let id = event.aggregate_id;
        match event.payload() {
            LoggingEvent::Logged(msg) => {
                if msg.contains("fail_projection") {
                    return Err(LoggingError::Domain(msg.clone()));
                }
                println!("Logged via projector from {}: {}", id, msg);
            }
        }
        Ok(())
    }
}

// Here's a simple side-effect policy which carries out some side effect.
// This is also failable - in a similar way to the projection above, however,
// if this fails, the event is still persisted, and then the aggregate state
// is lost. The largest difference between a failure in a projection and a
// failure in a policy, from an aggregate perspective, is that a failed projection
// stops the event from being persisted to the event store, whereas a failure in
// a policy does not.
#[derive(Clone)]
pub struct LoggingEventHandler;

#[async_trait]
impl EventHandler<LoggingAggregate> for LoggingEventHandler {
    async fn handle(&self, event: &StoreEvent<LoggingEvent>) {
        let id = event.aggregate_id;
        match event.payload() {
            LoggingEvent::Logged(msg) => {
                if msg.contains("fail_policy") {
                    return;
                }
                println!("Logged via policy from {}: {}", id, msg);
            }
        };
    }
}

// A simple error enum for event processing errors
#[derive(Debug, Error)]
pub enum LoggingError {
    #[error("[Err {0}]")]
    Domain(String),

    #[error(transparent)]
    Json(#[from] esrs::error::JsonError),

    #[error(transparent)]
    Sql(#[from] esrs::error::SqlxError),
}

// The events to be processed
#[derive(Serialize, Deserialize, Debug)]
pub enum LoggingEvent {
    Logged(String),
}

// The commands received by the application, which will produce the events
pub enum LoggingCommand {
    Log(String),
}
