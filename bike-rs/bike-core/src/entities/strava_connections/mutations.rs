use super::*;
use crate::entities::strava_connections;
use crate::strava::client::{
    StravaAthleteSummary, StravaAuthorizationTokenResponse, StravaRefreshTokenResponse,
};
use crate::workflow_error::WorkflowError as AppError;

impl Model {
    pub(crate) async fn connect_from_token(
        db: &DatabaseConnection,
        user_id: i32,
        token_response: StravaAuthorizationTokenResponse,
    ) -> Result<strava_connections::Model, AppError> {
        if let Some(existing_for_athlete) =
            strava_connections::Model::for_athlete(db, token_response.athlete.id).await?
        {
            if existing_for_athlete.user_id != user_id {
                return Err(AppError::bad_request(
                    "That Strava athlete is already connected to another Bike account",
                ));
            }
        }

        let expires_at = token_expiration(token_response.expires_at)?;

        let existing = Self::for_user(db, user_id).await?;
        let reconnect = existing.is_some();
        let active_model = match existing {
            Some(existing) => {
                let mut active: ActiveModel = existing.into();
                active.created_at = Set(Utc::now());
                active.disconnect_requested_at = Set(None);
                active
            }
            None => ActiveModel {
                user_id: Set(user_id),
                ..Default::default()
            },
        };
        let active_model = ActiveModel {
            scopes: Set(token_response.scope),
            ..active_model.with_athlete(token_response.athlete)
        }
        .with_tokens(
            &token_response.access_token,
            &token_response.refresh_token,
            expires_at,
        )?;
        if reconnect {
            active_model.update(db).await.map_err(AppError::from)
        } else {
            active_model.insert(db).await.map_err(AppError::from)
        }
    }

    pub(crate) async fn refresh_credentials(
        self,
        db: &DatabaseConnection,
        token_response: StravaRefreshTokenResponse,
    ) -> Result<Self, AppError> {
        let expires_at = token_expiration(token_response.expires_at)?;
        let mut active_model = ActiveModel {
            scopes: Set(token_response.scope.unwrap_or_else(|| self.scopes.clone())),
            ..self.into()
        }
        .with_tokens(
            &token_response.access_token,
            &token_response.refresh_token,
            expires_at,
        )?;
        if let Some(athlete) = token_response.athlete {
            active_model = active_model.with_athlete(athlete);
        }
        active_model.update(db).await.map_err(AppError::from)
    }
}

fn token_expiration(timestamp: i64) -> Result<DateTime<Utc>, AppError> {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .ok_or_else(|| AppError::internal("Strava returned an invalid token expiration"))
}

impl ActiveModel {
    fn with_athlete(self, athlete: StravaAthleteSummary) -> Self {
        Self {
            athlete_id: Set(athlete.id),
            athlete_username: Set(athlete.username),
            athlete_first_name: Set(athlete.firstname),
            athlete_last_name: Set(athlete.lastname),
            athlete_profile_medium_url: Set(athlete.profile_medium),
            ..self
        }
    }

    fn with_tokens(
        self,
        access_token: &str,
        refresh_token: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, AppError> {
        Ok(Self {
            access_token: Set(crate::strava::credentials::seal(access_token.as_bytes())?),
            refresh_token: Set(crate::strava::credentials::seal(refresh_token.as_bytes())?),
            expires_at: Set(expires_at),
            ..self
        })
    }
}
