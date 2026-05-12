use crate::app_error;
use crate::controllers::activities;
use crate::controllers::activity_imports;
use crate::controllers::admin;
use crate::controllers::fitness;
use crate::controllers::segments;
use crate::controllers::strava;
use crate::controllers::user_preferences;
use kaleido::auth::openapi as auth_openapi;
use kaleido::glass::openapi as glass_openapi;
use kaleido::glass::SecurityAddon;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        auth_openapi::paths::register,
        auth_openapi::paths::login,
        auth_openapi::paths::current,
        auth_openapi::paths::refresh,
        auth_openapi::paths::logout,
        auth_openapi::paths::verify_email,
        auth_openapi::paths::resend_confirmation,
        auth_openapi::paths::forgot_password,
        auth_openapi::paths::reset_password,
        auth_openapi::paths::oauth_authorize,
        auth_openapi::paths::oauth_callback,
        admin::backfill_analytics,
        admin::regenerate_user_segments,
        admin::reprocess_user_activity_imports,
        activities::list_activities,
        activities::get_activity,
        activities::delete_activity,
        activities::regenerate_activity,
        fitness::get_fitness_freshness,
        activity_imports::list_activity_imports,
        activity_imports::list_activity_archive_import_jobs,
        activity_imports::get_activity_archive_import_job,
        activity_imports::upload_activity_import,
        activity_imports::import_activity_archive_from_url,
        strava::begin_connect,
        strava::get_connection,
        strava::queue_sync,
        strava::disconnect_connection,
        strava::handle_callback,
        segments::list_segments,
        segments::get_segment,
        segments::import_segment,
        user_preferences::get_preferences,
        user_preferences::update_preferences,
        glass_openapi::paths::public_flags,
        glass_openapi::paths::list_flags,
        glass_openapi::paths::update_flag,
    ),
    components(
        schemas(
            app_error::ApiErrorResponse,
            admin::AnalyticsBackfillResponse,
            admin::RegenerateUserSegmentsRequest,
            admin::RegenerateUserSegmentsResponse,
            admin::ReprocessUserActivityImportsRequest,
            admin::ReprocessUserActivityImportsResponse,
            activities::ActivityResponse,
            activities::ActivitySegmentEffort,
            fitness::FitnessFreshnessPoint,
            fitness::FitnessFreshnessResponse,
            crate::training_profile::ActivityHeartRateZoneSummary,
            crate::activity_details::ActivityLap,
            crate::activity_details::ActivityChartPoint,
            crate::activity_details::ActivityRoutePoint,
            activity_imports::ActivityImportResponse,
            activity_imports::ArchiveUrlImportRequest,
            activity_imports::ActivityArchiveImportJobResponse,
            strava::StravaAuthorizeResponse,
            strava::StravaConnectionResponse,
            crate::archive_import::ActivityArchiveImportResponse,
            segments::SegmentResponse,
            segments::SegmentEffortResponse,
            user_preferences::UserPreferencesResponse,
            user_preferences::UpdateUserPreferencesRequest,
            auth_openapi::schemas::MessageResponse,
            auth_openapi::schemas::RegisterRequest,
            auth_openapi::schemas::RegisterResponse,
            auth_openapi::schemas::LoginRequest,
            auth_openapi::schemas::UserResponse,
            auth_openapi::schemas::ResendConfirmationRequest,
            auth_openapi::schemas::ForgotPasswordRequest,
            auth_openapi::schemas::ResetPasswordRequest,
            glass_openapi::schemas::PublicFlagResponse,
            glass_openapi::schemas::FeatureFlagResponse,
            glass_openapi::schemas::UpdateFlagRequest,
            glass_openapi::schemas::PaginatedResponse<activities::ActivityResponse>,
            glass_openapi::schemas::PaginatedResponse<glass_openapi::schemas::FeatureFlagResponse>,
            glass_openapi::schemas::PaginatedResponse<glass_openapi::schemas::PublicFlagResponse>,
            kaleido::glass::data::pagination::PaginationParams,
        )
    ),
    tags(
        (name = "activities", description = "Normalized activity list and detail endpoints"),
        (name = "fitness", description = "Training load, fitness, fatigue, and form analytics"),
        (name = "activity-imports", description = "Manual activity upload endpoints"),
        (name = "strava", description = "Strava OAuth connection and activity sync endpoints"),
        (name = "segments", description = "Manual segment import and effort comparison endpoints"),
        (name = "preferences", description = "Authenticated Bike user preferences"),
        (name = "admin", description = "Admin-only endpoints"),
        (name = "auth", description = "Authentication and user management"),
        (name = "flags", description = "Feature flags"),
        (name = "oauth", description = "OAuth authentication")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
