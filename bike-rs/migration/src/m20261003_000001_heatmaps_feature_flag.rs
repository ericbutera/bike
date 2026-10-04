use sea_orm_migration::{prelude::*, sea_query::OnConflict};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(FeatureFlags::Table)
                    .columns([
                        FeatureFlags::FeatureKey,
                        FeatureFlags::Enabled,
                        FeatureFlags::Description,
                    ])
                    .values_panic([
                        "heatmaps".into(),
                        false.into(),
                        "Show personal activity heatmaps".into(),
                    ])
                    .on_conflict(
                        OnConflict::column(FeatureFlags::FeatureKey)
                            .do_nothing()
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .exec_stmt(
                Query::delete()
                    .from_table(FeatureFlags::Table)
                    .and_where(Expr::col(FeatureFlags::FeatureKey).eq("heatmaps"))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum FeatureFlags {
    Table,
    FeatureKey,
    Enabled,
    Description,
}
