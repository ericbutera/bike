use super::*;
use chrono::{DateTime, Utc};
use sea_orm::{PaginatorTrait, QueryFilter, QueryOrder};
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in=Query)]
pub struct TaskListQuery {
    /// Exact request, trace, or pipeline run ID.
    pub correlation_id: Option<String>,
    pub task_type: Option<String>,
    pub status: Option<String>,
    pub error: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}
fn default_page() -> u64 {
    1
}
fn default_per_page() -> u64 {
    20
}
impl Model {
    pub async fn admin_page(db: &DatabaseConnection, params: &TaskListQuery) -> Result<(Vec<Self>, i64), DbErr> {
        let page = std::cmp::max(params.page, 1);
        let per_page = params.per_page.clamp(1, 100);
        let mut query = Entity::find().order_by_desc(Column::CreatedAt);

        if let Some(id) = &params.correlation_id {
            query = Model::with_correlation(query, id);
        }

        if let Some(ref t) = params.task_type {
            query = query.filter(Column::TaskType.eq(t.clone()));
        }
        if let Some(ref s) = params.status {
            query = query.filter(Column::Status.eq(s.clone()));
        }
        if let Some(ref e) = params.error {
            query = query.filter(Column::Error.contains(e.as_str()));
        }
        if let Some(ref d) = params.from_date {
            if let Ok(parsed) = DateTime::parse_from_rfc3339(d) {
                query = query.filter(Column::CreatedAt.gte(parsed.with_timezone(&Utc)));
            }
        }
        if let Some(ref d) = params.to_date {
            if let Ok(parsed) = DateTime::parse_from_rfc3339(d) {
                query = query.filter(Column::CreatedAt.lte(parsed.with_timezone(&Utc)));
            }
        }

        let paginator = query.paginate(db, per_page);
        let total = paginator.num_items().await? as i64;
        let items = paginator.fetch_page(page - 1).await?;
        Ok((items, total))
    }
}
