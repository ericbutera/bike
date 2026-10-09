use sea_orm::{entity::ColumnTrait, EntityTrait, Order, QueryOrder, Select};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::{IntoParams, ToSchema};

/// Generic sort order used across the API
#[derive(Debug, Deserialize, Serialize, ToSchema, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl std::str::FromStr for SortOrder {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "asc" | "ascending" => Ok(SortOrder::Asc),
            "desc" | "descending" => Ok(SortOrder::Desc),
            _ => Err(()),
        }
    }
}

/// Standard sort query parameters
#[derive(Debug, Default, Deserialize, IntoParams, ToSchema, Clone)]
pub struct SortParams {
    /// Field to sort by (string; controllers may map to enums)
    #[into_params(parameter(inline))]
    #[serde(default)]
    pub sort_by: Option<String>,

    /// Sort order (string: "asc" or "desc")
    #[into_params(parameter(inline))]
    #[serde(default)]
    pub sort_order: Option<String>,
}

pub trait Sortable {
    type Column: ColumnTrait;
    fn to_column(&self) -> Self::Column;
}

// Helper trait to apply sorting to a query
pub trait SortQueryExt<E: EntityTrait> {
    fn apply_sort<S>(self, sort: &SortParams) -> Self
    where
        S: Sortable<Column = E::Column> + FromStr;
}

impl<E> SortQueryExt<E> for Select<E>
where
    E: EntityTrait,
{
    fn apply_sort<S>(self, sort: &SortParams) -> Self
    where
        S: Sortable<Column = E::Column> + FromStr,
    {
        // Try to parse the sort_by string into the entity-specific Sort enum
        let parsed: Option<S> = sort.sort_by.as_ref().and_then(|s| s.parse::<S>().ok());

        let column = match parsed {
            Some(s) => s.to_column(),
            None => return self,
        };

        let sea_order = match sort
            .sort_order
            .as_ref()
            .and_then(|s| s.parse::<SortOrder>().ok())
            .unwrap_or(SortOrder::Asc)
        {
            SortOrder::Asc => Order::Asc,
            SortOrder::Desc => Order::Desc,
        };

        self.order_by(column, sea_order)
    }
}

#[cfg(test)]
#[path = "sorting_tests.rs"]
mod tests;
