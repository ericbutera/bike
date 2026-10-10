use super::synthetic_read_allowed;
use axum::http::Method;

#[test]
fn synthetic_map_reads_allow_current_image_routes() {
    for variant in ["thumbnail", "full"] {
        let path = format!("/api/activity-map-images/{variant}/1");
        assert!(synthetic_read_allowed(&Method::GET, &path));
        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            assert!(!synthetic_read_allowed(&method, &path));
        }
    }
}

#[test]
fn synthetic_map_reads_reject_unrelated_and_unsupported_routes() {
    for path in [
        "/api/activity-map-images",
        "/api/activity-map-images/full",
        "/api/activity-map-images/full/2",
        "/api/activity-map-images/unknown/1",
        "/api/activity-map-images/full/1/extra",
        "/api/admin/users",
        "/api/auth/logout",
    ] {
        assert!(!synthetic_read_allowed(&Method::GET, path), "{path}");
    }
}
