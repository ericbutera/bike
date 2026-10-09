use super::*;
use crate::entities::activities;
use sea_orm::{DbBackend, QueryTrait};

enum ActivitySort {
    Title,
}

impl FromStr for ActivitySort {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "title" => Ok(Self::Title),
            _ => Err(()),
        }
    }
}

impl Sortable for ActivitySort {
    type Column = activities::Column;

    fn to_column(&self) -> Self::Column {
        activities::Column::Title
    }
}

#[test]
fn unit_happy_sort_orders_parse_supported_names() {
    for value in ["asc", "ASC", "ascending"] {
        assert_eq!(value.parse::<SortOrder>(), Ok(SortOrder::Asc));
    }
    for value in ["desc", "DESC", "descending"] {
        assert_eq!(value.parse::<SortOrder>(), Ok(SortOrder::Desc));
    }
}

#[test]
fn unit_happy_activity_query_sorts_by_selected_column() {
    let sort = SortParams {
        sort_by: Some("title".into()),
        sort_order: Some("descending".into()),
    };
    let query = activities::Entity::find()
        .apply_sort::<ActivitySort>(&sort)
        .build(DbBackend::Postgres)
        .to_string();
    assert!(query.ends_with("ORDER BY \"activities\".\"title\" DESC"));
}
