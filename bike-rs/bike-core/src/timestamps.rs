//! Database defaults own inserts; SeaORM's model hook touches ordinary updates.
macro_rules! impl_timestamps {
    ($active_model:ty) => {
        #[async_trait::async_trait]
        impl sea_orm::ActiveModelBehavior for $active_model {
            async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, sea_orm::DbErr>
            where
                C: sea_orm::ConnectionTrait,
            {
                if !insert {
                    self.updated_at = sea_orm::Set(chrono::Utc::now());
                }
                Ok(self)
            }
        }
    };
}

pub(crate) use impl_timestamps;

#[cfg(test)]
mod tests {
    use crate::{entities::activities, test_support};
    use chrono::Utc;
    use sea_orm::{ActiveModelBehavior, DbBackend, MockDatabase, NotSet, Set};

    #[tokio::test]
    async fn inserts_leave_omitted_timestamps_to_database_defaults() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let active = activities::ActiveModel {
            created_at: NotSet,
            updated_at: NotSet,
            ..test_support::new_activity()
        };
        let inserted = active.before_save(&db, true).await.unwrap();
        assert_eq!(inserted.created_at, NotSet);
        assert_eq!(inserted.updated_at, NotSet);
    }

    #[tokio::test]
    async fn partial_updates_touch_updated_at_without_setting_creation_or_business_fields() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let before = Utc::now();
        let active = activities::ActiveModel {
            id: Set(42),
            title: Set("Evening ride".into()),
            ..Default::default()
        };
        let updated = active.before_save(&db, false).await.unwrap();
        assert_eq!(updated.created_at, NotSet);
        assert_eq!(updated.sport, NotSet);
        assert_eq!(updated.title, Set("Evening ride".into()));
        let updated_at = updated.updated_at.unwrap();
        assert!(updated_at >= before && updated_at <= Utc::now());
    }
}
