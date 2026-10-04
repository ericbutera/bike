use sea_orm_migration::{prelude::*, sea_query::OnConflict};

const FLAG_ENHANCED_MAPS: &str = "enhanced_maps";
const FLAG_DESCRIPTION: &str = "Use enhanced basemaps and generated activity route images";

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
                FLAG_ENHANCED_MAPS.into(),
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
            .and_where(Expr::col(FeatureFlags::FeatureKey).eq(FLAG_ENHANCED_MAPS))
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
