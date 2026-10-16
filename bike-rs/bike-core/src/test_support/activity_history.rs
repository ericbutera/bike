use crate::entities::activity_event_history::{Event, Model};

pub fn activity_history_event() -> Event {
    let activity = super::activity();
    let mut event = Event::manual(&activity, &activity).expect("valid fixture event");
    event.id = "manual:fixture-change".into();
    event.occurred_at = super::timestamp();
    event.context = serde_json::json!({"actor_user_id": activity.user_id, "trace_context": {}});
    event
}

pub fn activity_history() -> Model {
    let event = activity_history_event();
    Model {
        id: event.id,
        activity_id: event.activity_id,
        user_id: event.user_id,
        source: event.source,
        occurred_at: event.occurred_at,
        recorded_at: super::timestamp(),
        before_json: event.before,
        after_json: event.after,
        context_json: event.context,
    }
}
