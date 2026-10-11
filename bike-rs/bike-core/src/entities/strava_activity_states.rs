use sea_orm::entity::prelude::*;
use sea_orm::{sea_query::OnConflict, ConnectionTrait, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_activity_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub athlete_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i64,
    pub connection_generation: DateTimeUtc,
    pub event_time: i64,
    pub applied_hash: Option<String>,
    pub deleted: bool,
    pub applied_at: DateTimeUtc,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
impl Model {
    pub async fn save(self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
        Entity::insert(ActiveModel {
            athlete_id: Set(self.athlete_id),
            activity_id: Set(self.activity_id),
            connection_generation: Set(self.connection_generation),
            event_time: Set(self.event_time),
            applied_hash: Set(self.applied_hash),
            deleted: Set(self.deleted),
            applied_at: Set(self.applied_at),
        })
        .on_conflict(
            OnConflict::columns([Column::AthleteId, Column::ActivityId])
                .update_columns([
                    Column::ConnectionGeneration,
                    Column::EventTime,
                    Column::AppliedHash,
                    Column::Deleted,
                    Column::AppliedAt,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;
        Ok(())
    }
}
