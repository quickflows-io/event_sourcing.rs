use std::convert::TryInto;

use chrono::Utc;
use sqlx::types::Json;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use esrs::sql::event::Event;
use esrs::store::postgres::{PgStore, PgStoreBuilder};
use esrs::store::{EventStore, StoreEvent};
use esrs::AggregateState;

use crate::common::{new_pool, BasicAggregate, BasicEvent};

#[path = "../common/lib.rs"]
mod common;

#[tokio::main]
async fn main() {
    let pool: Pool<Postgres> = new_pool().await;

    let store: PgStore<BasicAggregate> = PgStoreBuilder::new(pool.clone()).try_build().await.unwrap();

    let aggregate_id: Uuid = Uuid::new_v4();
    let mut aggregate_state: AggregateState<()> = AggregateState::with_id(aggregate_id);

    let event = BasicEvent {
        content: "insert event content".to_string(),
    };

    // Insert an event
    let events = store.persist(&mut aggregate_state, vec![event]).await.unwrap();
    let original_event_1 = events.first().unwrap();

    // Insert an event with given event id
    let event_id: Uuid = Uuid::new_v4();
    let original_payload_2 = BasicEvent {
        content: "insert event by id content".to_string(),
    };

    let query: String = format!(
        include_str!("../../src/esrs/sql/postgres/statements/insert.sql"),
        store.table_name()
    );

    let _ = sqlx::query(query.as_str())
        .bind(event_id)
        .bind(aggregate_id)
        .bind(Json(&original_payload_2))
        .bind(Utc::now())
        .bind(aggregate_state.next_sequence_number())
        .execute(&pool)
        .await
        .unwrap();

    // Get an event by event id
    let event: StoreEvent<BasicEvent> = get_event_by_event_id(event_id, store.table_name(), &pool)
        .await
        .unwrap();

    assert_eq!(event.payload.content, original_payload_2.content);

    // Get events by aggregate id
    let events = store.by_aggregate_id(aggregate_id).await.unwrap();
    let payloads: Vec<BasicEvent> = events.into_iter().map(|v| v.payload).collect();
    assert!(payloads.contains(original_event_1.payload()));
    assert!(payloads.contains(&original_payload_2));

    // Update event payload by event id
    let new_payload: BasicEvent = BasicEvent {
        content: "updated content".to_string(),
    };
    let query: String = format!("UPDATE {} SET payload = $2 WHERE id = $1", store.table_name());

    let _ = sqlx::query(query.as_str())
        .bind(event_id)
        .bind(Json(new_payload.clone()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        get_event_by_event_id(event_id, store.table_name(), &pool)
            .await
            .unwrap()
            .payload
            .content,
        new_payload.content
    );

    // Delete event by event id
    let query: String = format!("DELETE FROM {} WHERE id = $1", store.table_name());

    let _ = sqlx::query(query.as_str()).bind(event_id).execute(&pool).await.unwrap();

    assert!(get_event_by_event_id(event_id, store.table_name(), &pool)
        .await
        .is_none());

    // Delete all aggregate events by aggregate id
    store.delete(aggregate_id).await.unwrap();

    assert!(store.by_aggregate_id(aggregate_id).await.unwrap().is_empty());
}

async fn get_event_by_event_id(id: Uuid, table_name: &str, pool: &Pool<Postgres>) -> Option<StoreEvent<BasicEvent>> {
    let query: String = format!("SELECT * FROM {} WHERE id = $1", table_name);

    sqlx::query_as::<_, Event>(query.as_str())
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
        .map(|v| v.try_into().unwrap())
}