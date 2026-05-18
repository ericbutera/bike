use sea_orm_migration::{prelude::*, sea_query::OnConflict};

const FLAG_ACTIVITY_LIST_FULL_MAPS: &str = "activity_list_full_maps";
const FLAG_DESCRIPTION: &str = "Show a full route map for each activity in the activity feed list";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let insert = Query::insert()
            .into_table(FeatureFlags::Table)
            .columns([
                FeatureFlags::FeatureKey,
                FeatureFlags::Enabled,
                FeatureFlags::Description,
            ])
            .values_panic([
                FLAG_ACTIVITY_LIST_FULL_MAPS.into(),
                false.into(),
                FLAG_DESCRIPTION.into(),
            ])
            .on_conflict(
                OnConflict::column(FeatureFlags::FeatureKey)
                    .do_nothing()
                    .to_owned(),
            )
            .to_owned();

        manager.exec_stmt(insert).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let delete = Query::delete()
            .from_table(FeatureFlags::Table)
            .and_where(Expr::col(FeatureFlags::FeatureKey).eq(FLAG_ACTIVITY_LIST_FULL_MAPS))
            .to_owned();

        manager.exec_stmt(delete).await
    }
}

#[derive(DeriveIden)]
enum FeatureFlags {
    Table,
    FeatureKey,
    Enabled,
    Description,
}
