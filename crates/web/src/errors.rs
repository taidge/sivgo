//! API error types for the Sivgo web server.

use salvo::prelude::*;
use serde::Serialize;

/// Unified API error type.
///
/// Carries an HTTP status code and a human-readable message. When written
/// to a `Response`, it sets the status code and renders a JSON body of the
/// form `{"error": "..."}`.
#[derive(Debug, Serialize)]
pub struct ApiError {
    #[serde(skip)]
    pub status: u16,
    pub error: String,
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: 400,
            error: msg.into(),
        }
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self {
            status: 401,
            error: msg.into(),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: 404,
            error: msg.into(),
        }
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self {
            status: 409,
            error: msg.into(),
        }
    }

    pub fn rate_limited(msg: impl Into<String>) -> Self {
        Self {
            status: 429,
            error: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: 500,
            error: msg.into(),
        }
    }

    /// Write this error to a Salvo `Response`.
    pub fn write_to(self, res: &mut Response) {
        let code = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        res.status_code(code);
        res.render(Json(self));
    }
}
