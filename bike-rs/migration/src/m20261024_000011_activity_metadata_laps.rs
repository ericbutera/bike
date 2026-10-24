use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Activities {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ActivityMetadata {
    Table,
    ActivityId,
    Description,
    Location,
    PrivateNotes,
    GearId,
    GearName,
}

#[derive(DeriveIden)]
enum ActivityLaps {
    Table,
    ActivityId,
    Position,
    LapIndex,
    Title,
    StartOffsetSeconds,
    DurationSeconds,
    DistanceMeters,
    ElevationGainMeters,
    ElevationLossMeters,
    AverageSpeedMps,
    MaxSpeedMps,
    AverageHeartRateBpm,
    MaxHeartRateBpm,
    AverageCadenceRpm,
    MaxCadenceRpm,
    Calories,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(activity_metadata()).await?;
        manager.create_table(activity_laps()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ActivityLaps::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ActivityMetadata::Table).to_owned())
            .await
    }
}

fn activity_metadata() -> TableCreateStatement {
    Table::create()
        .table(ActivityMetadata::Table)
        .col(
            ColumnDef::new(ActivityMetadata::ActivityId)
                .integer()
                .not_null()
                .primary_key(),
        )
        .col(ColumnDef::new(ActivityMetadata::Description).text())
        .col(ColumnDef::new(ActivityMetadata::Location).text())
        .col(ColumnDef::new(ActivityMetadata::PrivateNotes).text())
        .col(ColumnDef::new(ActivityMetadata::GearId).text())
        .col(ColumnDef::new(ActivityMetadata::GearName).text())
        .foreign_key(
            ForeignKey::create()
                .from(ActivityMetadata::Table, ActivityMetadata::ActivityId)
                .to(Activities::Table, Activities::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

fn activity_laps() -> TableCreateStatement {
    Table::create()
        .table(ActivityLaps::Table)
        .col(ColumnDef::new(ActivityLaps::ActivityId).integer().not_null())
        .col(ColumnDef::new(ActivityLaps::Position).integer().not_null())
        .col(ColumnDef::new(ActivityLaps::LapIndex).integer().not_null())
        .col(ColumnDef::new(ActivityLaps::Title).text().not_null())
        .col(ColumnDef::new(ActivityLaps::StartOffsetSeconds).integer())
        .col(ColumnDef::new(ActivityLaps::DurationSeconds).integer())
        .col(ColumnDef::new(ActivityLaps::DistanceMeters).double())
        .col(ColumnDef::new(ActivityLaps::ElevationGainMeters).double())
        .col(ColumnDef::new(ActivityLaps::ElevationLossMeters).double())
        .col(ColumnDef::new(ActivityLaps::AverageSpeedMps).double())
        .col(ColumnDef::new(ActivityLaps::MaxSpeedMps).double())
        .col(ColumnDef::new(ActivityLaps::AverageHeartRateBpm).integer())
        .col(ColumnDef::new(ActivityLaps::MaxHeartRateBpm).integer())
        .col(ColumnDef::new(ActivityLaps::AverageCadenceRpm).integer())
        .col(ColumnDef::new(ActivityLaps::MaxCadenceRpm).integer())
        .col(ColumnDef::new(ActivityLaps::Calories).integer())
        .primary_key(
            Index::create()
                .col(ActivityLaps::ActivityId)
                .col(ActivityLaps::Position),
        )
        .check(Expr::col(ActivityLaps::Position).gte(0))
        .foreign_key(
            ForeignKey::create()
                .from(ActivityLaps::Table, ActivityLaps::ActivityId)
                .to(Activities::Table, Activities::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}
