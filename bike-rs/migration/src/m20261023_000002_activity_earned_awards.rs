use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ActivityAwardOutputs {
    Table,
    ActivityId,
    UserId,
}

#[derive(DeriveIden)]
enum ActivityEarnedAwards {
    Table,
    ActivityId,
    Position,
    DisplayPosition,
    SegmentId,
    SegmentTitle,
    EffortIndex,
    OverallRank,
    PersonalRank,
    PersonalBestDurationSeconds,
}

#[derive(DeriveIden)]
enum Activities {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(outputs()).await?;
        manager.create_table(awards()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ActivityEarnedAwards::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ActivityAwardOutputs::Table).to_owned())
            .await
    }
}

fn outputs() -> TableCreateStatement {
    use ActivityAwardOutputs as O;
    Table::create()
        .table(O::Table)
        .col(ColumnDef::new(O::ActivityId).integer().not_null().primary_key())
        .col(ColumnDef::new(O::UserId).integer().not_null())
        .foreign_key(
            ForeignKey::create()
                .from(O::Table, O::ActivityId)
                .to(Activities::Table, Activities::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

fn awards() -> TableCreateStatement {
    use ActivityEarnedAwards as A;
    Table::create()
        .table(A::Table)
        .col(ColumnDef::new(A::ActivityId).integer().not_null())
        .col(ColumnDef::new(A::Position).integer().not_null())
        .col(ColumnDef::new(A::DisplayPosition).integer().not_null())
        .col(ColumnDef::new(A::SegmentId).integer().not_null())
        .col(ColumnDef::new(A::SegmentTitle).text().not_null())
        .col(ColumnDef::new(A::EffortIndex).integer().not_null())
        .col(ColumnDef::new(A::OverallRank).integer())
        .col(ColumnDef::new(A::PersonalRank).integer())
        .col(ColumnDef::new(A::PersonalBestDurationSeconds).integer())
        .primary_key(Index::create().col(A::ActivityId).col(A::Position))
        .index(Index::create().unique().col(A::ActivityId).col(A::DisplayPosition))
        .index(
            Index::create()
                .unique()
                .col(A::ActivityId)
                .col(A::SegmentId)
                .col(A::EffortIndex),
        )
        .foreign_key(
            ForeignKey::create()
                .from(A::Table, A::ActivityId)
                .to(ActivityAwardOutputs::Table, ActivityAwardOutputs::ActivityId)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}
