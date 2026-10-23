use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Segments {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum SegmentEfforts {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum SegmentOverallAchievements {
    Table,
    SegmentId,
    UserId,
    Position,
    EffortId,
    DurationSeconds,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(achievements()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SegmentOverallAchievements::Table).to_owned())
            .await
    }
}

fn achievements() -> TableCreateStatement {
    use SegmentOverallAchievements as A;
    Table::create()
        .table(A::Table)
        .col(ColumnDef::new(A::SegmentId).integer().not_null())
        .col(ColumnDef::new(A::UserId).integer().not_null())
        .col(ColumnDef::new(A::Position).integer().not_null())
        .col(ColumnDef::new(A::EffortId).integer().not_null().unique_key())
        .col(ColumnDef::new(A::DurationSeconds).integer().not_null())
        .primary_key(Index::create().col(A::SegmentId).col(A::Position))
        .index(Index::create().unique().col(A::SegmentId).col(A::UserId))
        .check(Expr::col(A::Position).between(1, 10))
        .foreign_key(
            ForeignKey::create()
                .from(A::Table, A::SegmentId)
                .to(Segments::Table, Segments::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .foreign_key(
            ForeignKey::create()
                .from(A::Table, A::UserId)
                .to(Users::Table, Users::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .foreign_key(
            ForeignKey::create()
                .from(A::Table, A::EffortId)
                .to(SegmentEfforts::Table, SegmentEfforts::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}
