use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityImportArtifacts {
    Table,
    Id,
    ActivityImportId,
    UserId,
    ArtifactKind,
    Format,
    SourceQuality,
    OriginalFilename,
    StoragePath,
    SizeBytes,
    MimeType,
    ChecksumSha256,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ActivityImports {
    Table,
    Id,
    UserId,
    Source,
    Format,
    OriginalFilename,
    StoragePath,
    SizeBytes,
    MimeType,
}

const STRAVA_SYNC_SOURCE: &str = "strava_sync";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ActivityImportArtifacts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::ActivityImportId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::ArtifactKind)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::Format)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::SourceQuality)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::OriginalFilename)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::StoragePath)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::SizeBytes)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::MimeType)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::ChecksumSha256)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(ActivityImportArtifacts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-activity-import-artifacts-import")
                            .from(
                                ActivityImportArtifacts::Table,
                                ActivityImportArtifacts::ActivityImportId,
                            )
                            .to(ActivityImports::Table, ActivityImports::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-activity-import-artifacts-import-id")
                    .table(ActivityImportArtifacts::Table)
                    .col(ActivityImportArtifacts::ActivityImportId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-activity-import-artifacts-user-kind-quality")
                    .table(ActivityImportArtifacts::Table)
                    .col(ActivityImportArtifacts::UserId)
                    .col(ActivityImportArtifacts::ArtifactKind)
                    .col(ActivityImportArtifacts::SourceQuality)
                    .to_owned(),
            )
            .await?;

        backfill_existing_import_artifacts(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ActivityImportArtifacts::Table)
                    .to_owned(),
            )
            .await
    }
}

async fn backfill_existing_import_artifacts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(ActivityImports::Id)
        .column(ActivityImports::UserId)
        .column(ActivityImports::Source)
        .column(ActivityImports::Format)
        .column(ActivityImports::OriginalFilename)
        .column(ActivityImports::StoragePath)
        .column(ActivityImports::SizeBytes)
        .column(ActivityImports::MimeType)
        .from(ActivityImports::Table)
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let import_id: i32 = row.try_get("", "id")?;
        let user_id: i32 = row.try_get("", "user_id")?;
        let source: String = row.try_get("", "source")?;
        let format: String = row.try_get("", "format")?;
        let original_filename: String = row.try_get("", "original_filename")?;
        let storage_path: String = row.try_get("", "storage_path")?;
        let size_bytes: i64 = row.try_get("", "size_bytes")?;
        let mime_type: Option<String> = row.try_get("", "mime_type")?;
        let is_legacy_strava_tcx = source == STRAVA_SYNC_SOURCE && format == "tcx";
        let artifact_kind = if is_legacy_strava_tcx {
            "generated_export"
        } else {
            "original"
        };
        let source_quality = if is_legacy_strava_tcx {
            "generated_tcx"
        } else {
            match format.as_str() {
                "fit" => "fit_original",
                "tcx" => "tcx_original",
                "gpx" => "gpx_original",
                _ => "unknown_original",
            }
        };

        let insert = Query::insert()
            .into_table(ActivityImportArtifacts::Table)
            .columns([
                ActivityImportArtifacts::ActivityImportId,
                ActivityImportArtifacts::UserId,
                ActivityImportArtifacts::ArtifactKind,
                ActivityImportArtifacts::Format,
                ActivityImportArtifacts::SourceQuality,
                ActivityImportArtifacts::OriginalFilename,
                ActivityImportArtifacts::StoragePath,
                ActivityImportArtifacts::SizeBytes,
                ActivityImportArtifacts::MimeType,
                ActivityImportArtifacts::ChecksumSha256,
            ])
            .values_panic([
                import_id.into(),
                user_id.into(),
                artifact_kind.into(),
                format.into(),
                source_quality.into(),
                original_filename.into(),
                storage_path.into(),
                size_bytes.into(),
                mime_type.into(),
                "".into(),
            ])
            .to_owned();

        connection.execute(&insert).await?;
    }

    Ok(())
}
