use super::activity_imports;
use crate::activity_import_pipeline::{checksum_sha256_hex, ActivityImportArtifactPayload};
use crate::entities::activity_import_artifacts;
use crate::workflow_error::WorkflowError as AppError;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Set, TryIntoModel,
};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "activity_import_artifacts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub activity_import_id: i32,
    pub user_id: i32,
    pub artifact_kind: String,
    pub format: String,
    pub source_quality: String,
    pub original_filename: String,
    pub storage_path: String,
    pub size_bytes: i64,
    pub mime_type: Option<String>,
    pub checksum_sha256: String,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Entity {
    pub async fn downloadable_original(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
    ) -> Result<Option<Model>, DbErr> {
        Ok(Self::for_import(db, user_id, import_id)
            .await?
            .into_iter()
            .filter(Model::is_downloadable_original)
            .max_by_key(Model::download_quality))
    }

    pub async fn storage_paths(db: &impl ConnectionTrait) -> Result<Vec<String>, DbErr> {
        Self::find()
            .select_only()
            .column(Column::StoragePath)
            .into_tuple()
            .all(db)
            .await
    }

    pub async fn strava_sources(db: &impl ConnectionTrait) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::ArtifactKind.is_in(["original", "provider_payload"]))
            .filter(Column::Format.is_in(["json", "fit", "tcx", "gpx"]))
            .filter(
                Column::ActivityImportId.in_subquery(
                    activity_imports::Entity::find()
                        .select_only()
                        .column(activity_imports::Column::Id)
                        .filter(activity_imports::Column::Source.eq("strava_sync"))
                        .into_query(),
                ),
            )
            .all(db)
            .await
    }

    pub async fn strava_payload(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityImportId.eq(import_id))
            .filter(Column::ArtifactKind.eq("provider_payload"))
            .filter(Column::SourceQuality.eq("strava_streams"))
            .filter(Column::Format.eq("json"))
            .one(db)
            .await
    }

    pub async fn shares_path(db: &impl ConnectionTrait, artifact: &Model) -> Result<bool, DbErr> {
        Ok(Self::find()
            .filter(Column::StoragePath.eq(&artifact.storage_path))
            .filter(Column::Id.ne(artifact.id))
            .count(db)
            .await?
            > 0)
    }
    pub async fn generated_page(
        db: &impl ConnectionTrait,
        user_id: i32,
        after_id: i32,
        limit: u64,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.gt(after_id))
            .filter(Column::ArtifactKind.eq("generated_export"))
            .filter(Column::SourceQuality.eq("generated_tcx"))
            .order_by_asc(Column::Id)
            .limit(limit.clamp(1, 32))
            .all(db)
            .await
    }

    pub async fn store_strava_payload(
        db: &impl ConnectionTrait,
        import: &activity_imports::Model,
        storage_path: String,
        upload: &crate::activity_import_pipeline::ActivityUploadPayload,
    ) -> Result<Model, DbErr> {
        use sha2::{Digest, Sha256};
        let existing = Self::for_import(db, import.user_id, import.id)
            .await?
            .into_iter()
            .find(|artifact| {
                artifact.artifact_kind == "provider_payload"
                    && artifact.source_quality == "strava_streams"
            });
        let mut model: ActiveModel = existing.map(Into::into).unwrap_or_default();
        model.activity_import_id = Set(import.id);
        model.user_id = Set(import.user_id);
        model.artifact_kind = Set("provider_payload".into());
        model.source_quality = Set("strava_streams".into());
        model.format = Set(upload.format.clone());
        model.original_filename = Set(upload.original_filename.clone());
        model.storage_path = Set(storage_path);
        model.size_bytes = Set(upload.bytes.len() as i64);
        model.mime_type = Set(upload.mime_type.clone());
        model.checksum_sha256 = Set(hex::encode(Sha256::digest(&upload.bytes)));
        model.save(db).await?.try_into_model()
    }

    pub async fn for_import(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityImportId.eq(import_id))
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    pub async fn find_existing_original_by_checksum<C>(
        db: &C,
        user_id: i32,
        checksum: &str,
    ) -> Result<Option<activity_imports::Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        let artifact = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ArtifactKind.eq("original"))
            .filter(Column::ChecksumSha256.eq(checksum))
            .order_by_desc(Column::Id)
            .one(db)
            .await?;

        let Some(artifact) = artifact else {
            return Ok(None);
        };

        activity_imports::Entity::find_by_id(artifact.activity_import_id)
            .filter(activity_imports::Column::UserId.eq(user_id))
            .one(db)
            .await
    }
}

impl Model {
    fn is_downloadable_original(&self) -> bool {
        self.artifact_kind == "original" && matches!(self.format.as_str(), "fit" | "tcx" | "gpx")
    }

    fn download_quality(&self) -> u16 {
        match self.source_quality.as_str() {
            "fit_original" => 500,
            "tcx_original" => 400,
            "gpx_original" => 300,
            _ => match self.format.as_str() {
                "fit" => 490,
                "tcx" => 390,
                "gpx" => 290,
                _ => 0,
            },
        }
    }
}

crate::timestamps::impl_timestamps!(ActiveModel);

impl Model {
    pub async fn store_artifact(
        db: &impl sea_orm::ConnectionTrait,
        import: &activity_imports::Model,
        artifact: ActivityImportArtifactPayload,
        storage_path: String,
        size_bytes: i64,
    ) -> Result<activity_import_artifacts::Model, AppError> {
        let checksum_sha256 = checksum_sha256_hex(&artifact.bytes);

        activity_import_artifacts::ActiveModel {
            activity_import_id: Set(import.id),
            user_id: Set(import.user_id),
            artifact_kind: Set(artifact.artifact_kind),
            format: Set(artifact.format),
            source_quality: Set(artifact.source_quality),
            original_filename: Set(artifact.original_filename),
            storage_path: Set(storage_path),
            size_bytes: Set(size_bytes),
            mime_type: Set(artifact.mime_type),
            checksum_sha256: Set(checksum_sha256),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(AppError::from)
    }
}

impl Model {
    pub async fn copy_with_size(
        self,
        db: &impl ConnectionTrait,
        size_bytes: i64,
    ) -> Result<Self, DbErr> {
        let mut model: ActiveModel = self.into();
        model.id = sea_orm::ActiveValue::NotSet;
        model.size_bytes = Set(size_bytes);
        model.insert(db).await
    }
    pub async fn copy_to_import(
        self,
        db: &impl ConnectionTrait,
        import_id: i32,
    ) -> Result<Self, DbErr> {
        let mut model: ActiveModel = self.into();
        model.id = sea_orm::ActiveValue::NotSet;
        model.activity_import_id = Set(import_id);
        model.insert(db).await
    }
}
