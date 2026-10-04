use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum Activities {
    Table,
    ActivityType,
    IsRace,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .add_column(
                        ColumnDef::new(Activities::ActivityType)
                            .string_len(32)
                            .not_null()
                            .default("training"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .exec_stmt(
                Query::update()
                    .table(Activities::Table)
                    .value(Activities::ActivityType, "race")
                    .and_where(Expr::col(Activities::IsRace).eq(true))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .drop_column(Activities::IsRace)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .add_column(
                        ColumnDef::new(Activities::IsRace)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .exec_stmt(
                Query::update()
                    .table(Activities::Table)
                    .value(Activities::IsRace, true)
                    .and_where(Expr::col(Activities::ActivityType).eq("race"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .drop_column(Activities::ActivityType)
                    .to_owned(),
            )
            .await
    }
}
