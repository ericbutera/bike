use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum Activities {
    Table,
    Id,
    UserId,
    Source,
    OriginalFilename,
    SourceCorrelationId,
}

const STRAVA_SYNC_SOURCE: &str = "strava_sync";
const SOURCE_CORRELATION_INDEX_NAME: &str = "idx-activities-user-source-correlation-id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .add_column(
                        ColumnDef::new(Activities::SourceCorrelationId)
                            .string()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        backfill_strava_source_correlation_ids(manager).await?;

        manager
            .create_index(
                Index::create()
                    .name(SOURCE_CORRELATION_INDEX_NAME)
                    .table(Activities::Table)
                    .col(Activities::UserId)
                    .col(Activities::Source)
                    .col(Activities::SourceCorrelationId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(SOURCE_CORRELATION_INDEX_NAME)
                    .table(Activities::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .drop_column(Activities::SourceCorrelationId)
                    .to_owned(),
            )
            .await
    }
}

async fn backfill_strava_source_correlation_ids(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(Activities::Id)
        .column(Activities::OriginalFilename)
        .from(Activities::Table)
        .and_where(Expr::col(Activities::Source).eq(STRAVA_SYNC_SOURCE))
        .and_where(Expr::col(Activities::OriginalFilename).is_not_null())
        .and_where(Expr::col(Activities::SourceCorrelationId).is_null())
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let original_filename: String = row.try_get("", "original_filename")?;
        let Some(source_correlation_id) = strava_activity_id_from_filename(&original_filename)
        else {
            continue;
        };

        let update = Query::update()
            .table(Activities::Table)
            .value(Activities::SourceCorrelationId, source_correlation_id)
            .and_where(Expr::col(Activities::Id).eq(id))
            .to_owned();

        connection.execute(&update).await?;
    }

    Ok(())
}

fn strava_activity_id_from_filename(filename: &str) -> Option<String> {
    let stem = filename
        .rsplit_once('.')
        .map(|(value, _)| value)
        .unwrap_or(filename);
    let candidate = stem.rsplit_once('_')?.1;

    if candidate.chars().all(|character| character.is_ascii_digit()) {
        Some(candidate.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_strava_activity_id_from_filename() {
        assert_eq!(
            strava_activity_id_from_filename("Morning_Mountain_Bike_Ride_18468904796.tcx"),
            Some("18468904796".to_string())
        );
        assert_eq!(
            strava_activity_id_from_filename("strava_activity_123456789.tcx"),
            Some("123456789".to_string())
        );
        assert_eq!(strava_activity_id_from_filename("ride.fit"), None);
    }
}