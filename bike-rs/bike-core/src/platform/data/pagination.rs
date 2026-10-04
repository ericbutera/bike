use sea_orm::{DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, Select};
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Standard pagination query parameters
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct PaginationParams {
    /// Page number (1-based)
    #[into_params(parameter(inline))]
    #[serde(
        default = "default_page",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub page: i64,
    /// Number of items per page
    #[serde(
        default = "default_per_page",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    20
}

fn deserialize_number_from_string<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt {
        String(String),
        Int(i64),
    }

    match StringOrInt::deserialize(deserializer)? {
        StringOrInt::String(s) => s.parse().map_err(serde::de::Error::custom),
        StringOrInt::Int(i) => Ok(i),
    }
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 20,
        }
    }
}

impl PaginationParams {
    /// Return a clamped/normalized (page, per_page) tuple.
    /// Ensures `page >= 1` and `1 <= per_page <= 100`.
    pub fn normalized(&self) -> (i64, i64) {
        let page = self.page.max(1);
        let per_page = self.per_page.clamp(1, 100);
        (page, per_page)
    }

    /// Convenience accessors
    pub fn page(&self) -> i64 {
        self.page.max(1)
    }

    pub fn per_page(&self) -> i64 {
        self.per_page.clamp(1, 100)
    }
}

/// Metadata about the pagination state
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginationMetadata {
    /// Current page number
    pub page: i64,
    /// Number of items per page
    pub per_page: i64,
    /// Total number of items available
    pub total: i64,
    /// Total number of pages available
    pub total_pages: i64,
}

/// Standard paginated response wrapper
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginatedResponse<T> {
    /// Recordset
    pub data: Vec<T>,
    /// Pagination metadata
    pub metadata: PaginationMetadata,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, page: i64, per_page: i64, total: i64) -> Self {
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

    pub fn from_params(data: Vec<T>, total: i64, params: &PaginationParams) -> Self {
        let (page, per_page) = params.normalized();
        Self::new(data, page, per_page, total)
    }

    pub fn map<U, F>(self, f: F) -> PaginatedResponse<U>
    where
        F: FnMut(T) -> U,
    {
        PaginatedResponse {
            data: self.data.into_iter().map(f).collect(),
            metadata: self.metadata,
        }
    }
}

// We define the trait without a generic T on the trait name itself.
// This makes usage much cleaner: `.fetch_paginated(...)`
#[allow(async_fn_in_trait)]
pub trait Paginatable {
    type Item; // Associated type for the model being returned

    async fn fetch_paginated(
        self,
        db: &DatabaseConnection,
        params: &PaginationParams,
    ) -> Result<PaginatedResponse<Self::Item>, DbErr>;
}

// We implement it for Select<E>
impl<E> Paginatable for Select<E>
where
    E: EntityTrait + Send + Sync,
    E::Model: Send + Sync,
{
    type Item = E::Model;

    async fn fetch_paginated(
        self,
        db: &DatabaseConnection,
        params: &PaginationParams,
    ) -> Result<PaginatedResponse<Self::Item>, DbErr> {
        let (page, per_page) = params.normalized();

        let paginator = self.paginate(db, per_page as u64);
        let total = paginator.num_items().await? as i64;
        let items = paginator.fetch_page((page - 1) as u64).await?;

        Ok(PaginatedResponse::new(items, page, per_page, total))
    }
}
