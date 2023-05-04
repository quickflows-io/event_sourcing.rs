use sqlx::{Pool, Postgres};

use esrs::postgres::PgStore;
use esrs::Aggregate;

use crate::projector::CounterTransactionalEventHandler;
use crate::structs::{CounterCommand, CounterError, CounterEvent};

pub struct CounterAggregate {
    pub event_store: PgStore<Self>,
}

impl CounterAggregate {
    pub async fn new(pool: &Pool<Postgres>) -> Result<Self, CounterError> {
        let event_store: PgStore<CounterAggregate> = PgStore::new(pool.clone())
            .set_transactional_event_handlers(vec![Box::new(CounterTransactionalEventHandler)])
            .setup()
            .await?;

        Ok(Self { event_store })
    }
}

impl Aggregate for CounterAggregate {
    const NAME: &'static str = "counter";
    type State = ();
    type Command = CounterCommand;
    type Event = CounterEvent;
    type Error = CounterError;

    fn handle_command(_state: &Self::State, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            Self::Command::Increment => Ok(vec![Self::Event::Incremented]),
            Self::Command::Decrement => Ok(vec![Self::Event::Decremented]),
        }
    }

    fn apply_event(state: Self::State, _: Self::Event) -> Self::State {
        // Take no action as this aggregate has no in memory state - only the projection
        state
    }
}
