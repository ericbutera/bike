use cron::Schedule;
use std::future::Future;
use tracing::Instrument;

pub fn spawn_scheduler<F, Fut>(schedule_expression: Option<&str>, mut enqueue: F)
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
{
    let Some(expression) = schedule_expression else {
        return;
    };

    let expression = expression.to_string();
    tokio::spawn(async move {
        let schedule = match expression.parse::<Schedule>() {
            Ok(schedule) => schedule,
            Err(error) => {
                tracing::error!(%error, %expression, "invalid cron schedule expression");
                return;
            }
        };

        for datetime in schedule.upcoming(chrono::Utc) {
            let now = chrono::Utc::now();
            let sleep_for = datetime
                .signed_duration_since(now)
                .to_std()
                .unwrap_or_default();

            tokio::time::sleep(sleep_for).await;

            let span = tracing::info_span!(
                "background_task.schedule.enqueue",
                schedule.expression = expression.as_str(),
                schedule.due_at = %datetime,
            );

            if let Err(error) = enqueue().instrument(span).await {
                tracing::error!(%error, "scheduled enqueue failed");
            }
        }
    });
}
