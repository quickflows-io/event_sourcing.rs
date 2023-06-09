use std::sync::Arc;

use futures::lock::Mutex;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use esrs::manager::AggregateManager;
use esrs::store::postgres::{PgStore, PgStoreBuilder};
use esrs::store::EventStore;
use esrs::AggregateState;

use crate::aggregate::{SagaAggregate, SagaCommand, SagaEvent};
use crate::common::new_pool;
use crate::event_handler::SagaEventHandler;

mod aggregate;
#[path = "../common/lib.rs"]
mod common;
mod event_handler;

#[tokio::main]
async fn main() {
    let pool: Pool<Postgres> = new_pool().await;

    let side_effect_mutex: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    let store: PgStore<SagaAggregate> = PgStoreBuilder::new(pool.clone()).try_build().await.unwrap();

    let saga_event_handler: SagaEventHandler = SagaEventHandler {
        store: store.clone(),
        side_effect_mutex: side_effect_mutex.clone(),
    };

    store.add_event_handler(saga_event_handler).await;

    let manager: AggregateManager<PgStore<SagaAggregate>> = AggregateManager::new(store.clone());

    let state: AggregateState<()> = AggregateState::default();
    let id: Uuid = *state.id();

    manager
        .handle_command(state, SagaCommand::RequestMutation)
        .await
        .unwrap();

    let events = store.by_aggregate_id(id).await.unwrap();

    let payloads: Vec<_> = events.into_iter().map(|v| v.payload).collect();

    assert!(payloads.contains(&SagaEvent::MutationRequested));
    assert!(payloads.contains(&SagaEvent::MutationRegistered));

    let guard = side_effect_mutex.lock().await;
    assert!(*guard);
}