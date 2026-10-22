use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ActivityClimbs {
    Table,
    ElevationLossMeters,
    MaxGradePercent,
    Category,
    RoutePointsJson,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityClimbs::Table)
                    .add_column(ColumnDef::new(ActivityClimbs::ElevationLossMeters).double().null())
                    .add_column(ColumnDef::new(ActivityClimbs::MaxGradePercent).double().null())
                    .add_column(ColumnDef::new(ActivityClimbs::Category).text().null())
                    .add_column(
                        ColumnDef::new(ActivityClimbs::RoutePointsJson)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'[]'::jsonb")),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityClimbs::Table)
                    .drop_column(ActivityClimbs::RoutePointsJson)
                    .drop_column(ActivityClimbs::Category)
                    .drop_column(ActivityClimbs::MaxGradePercent)
                    .drop_column(ActivityClimbs::ElevationLossMeters)
                    .to_owned(),
            )
            .await
    }
}
