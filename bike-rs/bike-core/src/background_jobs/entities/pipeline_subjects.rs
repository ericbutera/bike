use sea_orm::{entity::prelude::*, sea_query::OnConflict, ConnectionTrait, QuerySelect, Set};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "processing_pipeline_subjects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub run_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub subject_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn from_task(
        db: &impl ConnectionTrait,
        run_id: &str,
        payload: &Json,
    ) -> Result<(), DbErr> {
        let data = payload.get("data").unwrap_or(payload);
        for (kind, key) in [
            ("activity", "activity_id"),
            ("import", "import_id"),
            ("archive", "job_id"),
        ] {
            if let Some(id) = data
                .get(key)
                .and_then(Json::as_i64)
                .and_then(|id| i32::try_from(id).ok())
            {
                Self::attach(db, run_id, kind, id).await?;
                if kind == "import" {
                    if let Some(activity_id) =
                        crate::entities::activity_imports::Entity::find_by_id(id)
                            .select_only()
                            .column(crate::entities::activity_imports::Column::ActivityId)
                            .into_tuple::<Option<i32>>()
                            .one(db)
                            .await?
                            .flatten()
                    {
                        Self::attach(db, run_id, "activity", activity_id).await?;
                    }
                }
            }
        }
        if let Some(activities) = data.get("activities").and_then(Json::as_array) {
            for activity in activities {
                if let Some(id) = activity
                    .get("activity_id")
                    .and_then(Json::as_i64)
                    .and_then(|id| i32::try_from(id).ok())
                {
                    Self::attach(db, run_id, "activity", id).await?;
                }
            }
        }
        Ok(())
    }
    pub async fn attach(
        db: &impl ConnectionTrait,
        run_id: &str,
        kind: &str,
        subject_id: i32,
    ) -> Result<(), DbErr> {
        Entity::insert(ActiveModel {
            run_id: Set(run_id.into()),
            kind: Set(kind.into()),
            subject_id: Set(subject_id),
        })
        .on_conflict(
            OnConflict::columns([Column::RunId, Column::Kind, Column::SubjectId])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Ok(())
    }
}
