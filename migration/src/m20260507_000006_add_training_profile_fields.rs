use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum Activities {
    Table,
    EstimatedFtpWatts,
    HeartRateZonesJson,
}

#[derive(Iden)]
enum UserPreferences {
    Table,
    EstimatedFtpWatts,
    HeartRateZoneBoundsJson,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .add_column(
                        ColumnDef::new(UserPreferences::EstimatedFtpWatts)
                            .integer()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(UserPreferences::HeartRateZoneBoundsJson)
                            .text()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .add_column(
                        ColumnDef::new(Activities::EstimatedFtpWatts)
                            .integer()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(Activities::HeartRateZonesJson)
                            .text()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .drop_column(Activities::HeartRateZonesJson)
                    .drop_column(Activities::EstimatedFtpWatts)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .drop_column(UserPreferences::HeartRateZoneBoundsJson)
                    .drop_column(UserPreferences::EstimatedFtpWatts)
                    .to_owned(),
            )
            .await
    }
}