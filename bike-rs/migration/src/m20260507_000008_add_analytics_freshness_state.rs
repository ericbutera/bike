use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum Segments {
    Table,
    LastActivityChangeAt,
}

#[derive(Iden)]
enum AnalyticsUserStates {
    Table,
    UserId,
    LastActivityChangeAt,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .add_column(
                        ColumnDef::new(Segments::LastActivityChangeAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AnalyticsUserStates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AnalyticsUserStates::UserId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AnalyticsUserStates::LastActivityChangeAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(AnalyticsUserStates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(AnalyticsUserStates::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AnalyticsUserStates::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .drop_column(Segments::LastActivityChangeAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
