//! Health-check endpoint.

use salvo::prelude::*;

/// `GET /api/v1/health`
///
/// Returns `{"status": "ok"}` with HTTP 200.
#[handler]
pub async fn health_check(res: &mut Response) {
    res.render(Json(serde_json::json!({
        "status": "ok",
        "service": "sivgo-server",
    })));
}
