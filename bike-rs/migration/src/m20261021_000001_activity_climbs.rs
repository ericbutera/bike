use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Activities {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ActivityClimbOutputs {
    Table,
    ActivityId,
    DetectorVersion,
    RecordingSha256,
    ObjectRef,
    ClimbCount,
}

#[derive(DeriveIden)]
enum ActivityClimbs {
    Table,
    ActivityId,
    ClimbNumber,
    StartRoutePointIndex,
    EndRoutePointIndex,
    StartElapsedSeconds,
    EndElapsedSeconds,
    DurationSeconds,
    DistanceMeters,
    ElevationGainMeters,
    AverageGradePercent,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(climb_outputs()).await?;
        manager.create_table(climbs()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ActivityClimbs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ActivityClimbOutputs::Table).to_owned())
            .await
    }
}

fn climb_outputs() -> TableCreateStatement {
    Table::create()
        .table(ActivityClimbOutputs::Table)
        .col(
            ColumnDef::new(ActivityClimbOutputs::ActivityId)
                .integer()
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(ActivityClimbOutputs::DetectorVersion)
                .integer()
                .not_null(),
        )
        .col(ColumnDef::new(ActivityClimbOutputs::RecordingSha256).text().not_null())
        .col(ColumnDef::new(ActivityClimbOutputs::ObjectRef).json_binary().not_null())
        .col(ColumnDef::new(ActivityClimbOutputs::ClimbCount).integer().not_null())
        .check(Expr::col(ActivityClimbOutputs::ClimbCount).gte(0))
        .foreign_key(
            ForeignKey::create()
                .from(ActivityClimbOutputs::Table, ActivityClimbOutputs::ActivityId)
                .to(Activities::Table, Activities::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

fn climbs() -> TableCreateStatement {
    Table::create()
        .table(ActivityClimbs::Table)
        .col(ColumnDef::new(ActivityClimbs::ActivityId).integer().not_null())
        .col(ColumnDef::new(ActivityClimbs::ClimbNumber).integer().not_null())
        .col(ColumnDef::new(ActivityClimbs::StartRoutePointIndex).integer())
        .col(ColumnDef::new(ActivityClimbs::EndRoutePointIndex).integer())
        .col(ColumnDef::new(ActivityClimbs::StartElapsedSeconds).integer().not_null())
        .col(ColumnDef::new(ActivityClimbs::EndElapsedSeconds).integer().not_null())
        .col(ColumnDef::new(ActivityClimbs::DurationSeconds).integer().not_null())
        .col(ColumnDef::new(ActivityClimbs::DistanceMeters).double().not_null())
        .col(ColumnDef::new(ActivityClimbs::ElevationGainMeters).double().not_null())
        .col(ColumnDef::new(ActivityClimbs::AverageGradePercent).double())
        .primary_key(
            Index::create()
                .col(ActivityClimbs::ActivityId)
                .col(ActivityClimbs::ClimbNumber),
        )
        .check(Expr::col(ActivityClimbs::ClimbNumber).gt(0))
        .foreign_key(
            ForeignKey::create()
                .from(ActivityClimbs::Table, ActivityClimbs::ActivityId)
                .to(ActivityClimbOutputs::Table, ActivityClimbOutputs::ActivityId)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}
