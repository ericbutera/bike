use crate::background_jobs::entities::background_tasks;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, JsonValue, NotSet,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

/// Storage trait for background tasks admin routes.
pub trait BackgroundTasksStorage: Send + Sync + 'static {
    fn db(&self) -> &DatabaseConnection;
}

/// Marker trait for types that verify admin authorization.
///
/// Implement this for your admin extractor so it can be used as the admin guard
/// for [`admin_routes`]. For example, with `auth::AdminUserContext`:
///
/// ```ignore
/// impl<S> background_jobs::admin::AdminVerified for auth::AdminUserContext<S> {}
/// ```
pub trait AdminVerified {}

/// Returns the pathless admin router for background task management.
///
/// Most applications should use [`api_routes`] instead so the standard
/// `/admin/tasks` path stays owned by Kaleido.
///
/// `A` must be an extractor that implements [`AdminVerified`], ensuring only
/// admin-authenticated requests reach the handlers.
///
/// # Example
///
/// ```ignore
/// use auth::AdminUserContext;
///
/// // In controllers/mod.rs:
/// impl background_jobs::admin::AdminVerified for AdminUserContext<AppStorage> {}
///
/// router.nest(
///     "/api",
///     background_jobs::admin::api_routes::<AppStorage, AdminUserContext<AppStorage>>(),
/// )
/// ```
pub fn admin_routes<S, A>() -> Router<Arc<S>>
where
    S: BackgroundTasksStorage + 'static,
    A: AdminVerified + axum::extract::FromRequestParts<Arc<S>> + Send + 'static,
    <A as axum::extract::FromRequestParts<Arc<S>>>::Rejection: IntoResponse,
{
    Router::new()
        .route("/", get(list_tasks::<S, A>))
        .route("/:id", get(get_task::<S, A>))
        .route("/:id/rerun", post(rerun_task::<S, A>))
        .route("/:id/cancel", post(cancel_task::<S, A>))
}

/// Returns the background task admin routes with their standard API path.
///
/// Mount at `/api`:
/// ```ignore
/// .nest("/api", background_jobs::admin::api_routes::<AppStorage, AdminUserContext<AppStorage>>())
/// ```
pub fn api_routes<S, A>() -> Router<Arc<S>>
where
    S: BackgroundTasksStorage + 'static,
    A: AdminVerified + axum::extract::FromRequestParts<Arc<S>> + Send + 'static,
    <A as axum::extract::FromRequestParts<Arc<S>>>::Rejection: IntoResponse,
{
    Router::new().nest("/admin/tasks", admin_routes::<S, A>())
}

#[derive(Debug)]
pub struct AdminTaskError {
    code: StatusCode,
    message: String,
}

impl AdminTaskError {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl From<sea_orm::DbErr> for AdminTaskError {
    fn from(e: sea_orm::DbErr) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        }
    }
}

impl IntoResponse for AdminTaskError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({ "error": self.message }));
        (self.code, body).into_response()
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TaskListQuery {
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

#[derive(Debug, Serialize, ToSchema)]
pub struct PaginationMetadata {
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
    pub total_pages: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub metadata: PaginationMetadata,
}

impl<T> PaginatedResponse<T> {
    fn new(data: Vec<T>, page: i64, per_page: i64, total: i64) -> Self {
        let total_pages = if per_page > 0 {
            (total as f64 / per_page as f64).ceil() as i64
        } else {
            0
        };
        Self {
            data,
            metadata: PaginationMetadata {
                page,
                per_page,
                total,
                total_pages,
            },
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskResponse {
    pub id: i32,
    pub task_type: String,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub error: Option<String>,
    pub result: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub scheduled_for: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl From<background_tasks::Model> for TaskResponse {
    fn from(m: background_tasks::Model) -> Self {
        Self {
            id: m.id,
            task_type: m.task_type,
            status: m.status,
            attempts: m.attempts,
            max_attempts: m.max_attempts,
            error: m.error,
            result: m.result,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
            scheduled_for: m.scheduled_for.map(|d| d.to_rfc3339()),
            started_at: m.started_at.map(|d| d.to_rfc3339()),
            completed_at: m.completed_at.map(|d| d.to_rfc3339()),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskDetailResponse {
    pub id: i32,
    pub task_type: String,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub error: Option<String>,
    pub result: Option<String>,
    pub payload: Option<JsonValue>,
    pub created_at: String,
    pub updated_at: String,
    pub scheduled_for: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl From<background_tasks::Model> for TaskDetailResponse {
    fn from(m: background_tasks::Model) -> Self {
        Self {
            id: m.id,
            task_type: m.task_type,
            status: m.status,
            attempts: m.attempts,
            max_attempts: m.max_attempts,
            error: m.error,
            result: m.result,
            payload: Some(m.payload),
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
            scheduled_for: m.scheduled_for.map(|d| d.to_rfc3339()),
            started_at: m.started_at.map(|d| d.to_rfc3339()),
            completed_at: m.completed_at.map(|d| d.to_rfc3339()),
        }
    }
}

#[utoipa::path(
    get,
    path = "/admin/tasks",
    operation_id = "admin_list_tasks",
    params(TaskListQuery),
    responses(
        (status = 200, description = "Admin list of background tasks", body = PaginatedResponse<TaskResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn list_tasks<S, A>(
    _admin: A,
    State(state): State<Arc<S>>,
    Query(params): Query<TaskListQuery>,
) -> Result<Json<PaginatedResponse<TaskResponse>>, AdminTaskError>
where
    S: BackgroundTasksStorage,
    A: AdminVerified,
{
    let db = BackgroundTasksStorage::db(&*state);
    let page = params.page.max(1);
    let per_page = params.per_page.clamp(1, 100);

    let mut query =
        background_tasks::Entity::find().order_by_desc(background_tasks::Column::CreatedAt);

    if let Some(ref t) = params.task_type {
        query = query.filter(background_tasks::Column::TaskType.eq(t.clone()));
    }
    if let Some(ref s) = params.status {
        query = query.filter(background_tasks::Column::Status.eq(s.clone()));
    }
    if let Some(ref e) = params.error {
        query = query.filter(background_tasks::Column::Error.contains(e.as_str()));
    }
    if let Some(ref d) = params.from_date {
        if let Ok(parsed) = DateTime::parse_from_rfc3339(d) {
            query =
                query.filter(background_tasks::Column::CreatedAt.gte(parsed.with_timezone(&Utc)));
        }
    }
    if let Some(ref d) = params.to_date {
        if let Ok(parsed) = DateTime::parse_from_rfc3339(d) {
            query =
                query.filter(background_tasks::Column::CreatedAt.lte(parsed.with_timezone(&Utc)));
        }
    }

    let paginator = query.paginate(db, per_page);
    let total = paginator.num_items().await? as i64;
    let items = paginator.fetch_page(page - 1).await?;
    let data: Vec<TaskResponse> = items.into_iter().map(TaskResponse::from).collect();

    Ok(Json(PaginatedResponse::new(
        data,
        page as i64,
        per_page as i64,
        total,
    )))
}

#[utoipa::path(
    get,
    path = "/admin/tasks/{id}",
    operation_id = "admin_get_task",
    params(
        ("id" = i32, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Background task detail", body = TaskDetailResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn get_task<S, A>(
    _admin: A,
    State(state): State<Arc<S>>,
    Path(id): Path<i32>,
) -> Result<Json<TaskDetailResponse>, AdminTaskError>
where
    S: BackgroundTasksStorage,
    A: AdminVerified,
{
    let db = BackgroundTasksStorage::db(&*state);
    let task = background_tasks::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AdminTaskError::not_found("Task not found"))?;

    Ok(Json(TaskDetailResponse::from(task)))
}

#[utoipa::path(
    post,
    path = "/admin/tasks/{id}/rerun",
    operation_id = "admin_rerun_task",
    params(
        ("id" = i32, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task queued again", body = TaskResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn rerun_task<S, A>(
    _admin: A,
    State(state): State<Arc<S>>,
    Path(id): Path<i32>,
) -> Result<Json<TaskResponse>, AdminTaskError>
where
    S: BackgroundTasksStorage,
    A: AdminVerified,
{
    let db = BackgroundTasksStorage::db(&*state);
    let task = background_tasks::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AdminTaskError::not_found("Task not found"))?;

    let now = Utc::now();
    let created = background_tasks::ActiveModel {
        id: NotSet,
        task_type: Set(task.task_type),
        payload: Set(task.payload),
        status: Set("pending".to_string()),
        attempts: Set(0),
        max_attempts: Set(task.max_attempts),
        error: Set(None),
        result: Set(None),
        scheduled_for: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        started_at: Set(None),
        completed_at: Set(None),
    }
    .insert(db)
    .await?;

    Ok(Json(TaskResponse::from(created)))
}

#[utoipa::path(
    post,
    path = "/admin/tasks/{id}/cancel",
    operation_id = "admin_cancel_task",
    params(
        ("id" = i32, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task canceled", body = TaskResponse),
        (status = 400, description = "Task is not processing"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn cancel_task<S, A>(
    _admin: A,
    State(state): State<Arc<S>>,
    Path(id): Path<i32>,
) -> Result<Json<TaskResponse>, AdminTaskError>
where
    S: BackgroundTasksStorage,
    A: AdminVerified,
{
    let db = BackgroundTasksStorage::db(&*state);
    let task = background_tasks::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AdminTaskError::not_found("Task not found"))?;

    if task.status != "processing" {
        return Err(AdminTaskError::bad_request(
            "Only processing tasks can be canceled",
        ));
    }

    let now = Utc::now();
    let mut active: background_tasks::ActiveModel = task.into();
    active.status = Set("canceled".to_string());
    active.error = Set(Some("Canceled by admin".to_string()));
    active.completed_at = Set(Some(now));
    active.updated_at = Set(now);

    let updated = active.update(db).await?;
    Ok(Json(TaskResponse::from(updated)))
}
