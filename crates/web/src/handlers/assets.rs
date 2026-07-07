//! Asset upload/download handlers.

use base64::{engine::general_purpose, Engine as _};
use salvo::prelude::*;
use uuid::Uuid;

use crate::assets::AssetStore;
use crate::auth;
use crate::errors::ApiError;

/// `GET /api/v1/assets`
///
/// Lists all assets owned by the authenticated user.
#[handler]
pub async fn list_assets(depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<AssetStore>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("AssetStore unavailable").write_to(res);
            return;
        }
    };
    let user_id = match auth::get_user_id(depot) {
        Some(id) => id,
        None => {
            ApiError::unauthorized("Not authenticated").write_to(res);
            return;
        }
    };

    let assets = store.list_assets(user_id);
    res.render(Json(assets));
}

/// Request body for uploading an asset.
#[derive(Debug, serde::Deserialize)]
struct UploadAssetRequest {
    filename: String,
    content_type: String,
    /// Base64-encoded data.
    data: String,
}

/// `POST /api/v1/assets`
///
/// Uploads a new asset. Expects a JSON body with filename, content_type,
/// and base64-encoded data.
#[handler]
pub async fn upload_asset(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<AssetStore>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("AssetStore unavailable").write_to(res);
            return;
        }
    };
    let user_id = match auth::get_user_id(depot) {
        Some(id) => id,
        None => {
            ApiError::unauthorized("Not authenticated").write_to(res);
            return;
        }
    };

    let body: UploadAssetRequest = match req.parse_json().await {
        Ok(b) => b,
        Err(e) => {
            ApiError::bad_request(format!("Invalid request body: {e}")).write_to(res);
            return;
        }
    };

    if body.filename.is_empty() {
        ApiError::bad_request("filename is required").write_to(res);
        return;
    }
    if body.content_type.is_empty() {
        ApiError::bad_request("content_type is required").write_to(res);
        return;
    }

    // Bound the upload size before decoding to limit memory-exhaustion DoS.
    // Base64 inflates by ~4/3, so a 24 MiB payload decodes to ~18 MiB.
    const MAX_ASSET_B64_BYTES: usize = 24 * 1024 * 1024;
    if body.data.len() > MAX_ASSET_B64_BYTES {
        ApiError::bad_request("asset data exceeds 24 MiB limit").write_to(res);
        return;
    }

    let data = match general_purpose::STANDARD.decode(body.data.trim()) {
        Ok(data) if !data.is_empty() => data,
        Ok(_) => {
            ApiError::bad_request("data is required").write_to(res);
            return;
        }
        Err(e) => {
            ApiError::bad_request(format!("data must be valid base64: {e}")).write_to(res);
            return;
        }
    };
    let info = store.upload_asset(user_id, &body.filename, &body.content_type, data);

    res.status_code(StatusCode::CREATED);
    res.render(Json(info));
}

/// `GET /api/v1/assets/{id}`
///
/// Downloads an asset by id.
#[handler]
pub async fn download_asset(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<AssetStore>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("AssetStore unavailable").write_to(res);
            return;
        }
    };
    let user_id = match auth::get_user_id(depot) {
        Some(id) => id,
        None => {
            ApiError::unauthorized("Not authenticated").write_to(res);
            return;
        }
    };

    let asset_id = match parse_uuid_param(req, "id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid asset id").write_to(res);
            return;
        }
    };

    match store.download_asset(asset_id) {
        Some(asset) if asset.owner_id == user_id => {
            res.headers_mut().insert(
                "content-type",
                asset
                    .content_type
                    .parse()
                    .unwrap_or_else(|_| "application/octet-stream".parse().unwrap()),
            );
            // Sanitize the user-controlled filename before placing it in a
            // response header: strip quotes, backslashes and control chars
            // (CR/LF) to prevent header injection, and never unwrap on the
            // resulting HeaderValue.
            let safe_name: String = asset
                .filename
                .chars()
                .map(|c| {
                    if c == '"' || c == '\\' || c.is_control() {
                        '_'
                    } else {
                        c
                    }
                })
                .collect();
            if let Ok(value) = format!("attachment; filename=\"{}\"", safe_name).parse() {
                res.headers_mut().insert("content-disposition", value);
            }
            res.body(asset.data);
        }
        Some(_) | None => {
            ApiError::not_found("Asset not found").write_to(res);
        }
    }
}

/// `DELETE /api/v1/assets/{id}`
///
/// Deletes an asset by id.
#[handler]
pub async fn delete_asset(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<AssetStore>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("AssetStore unavailable").write_to(res);
            return;
        }
    };
    let user_id = match auth::get_user_id(depot) {
        Some(id) => id,
        None => {
            ApiError::unauthorized("Not authenticated").write_to(res);
            return;
        }
    };

    let asset_id = match parse_uuid_param(req, "id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid asset id").write_to(res);
            return;
        }
    };

    match store.download_asset(asset_id) {
        Some(asset) if asset.owner_id == user_id => match store.delete_asset(asset_id) {
            Ok(()) => {
                res.status_code(StatusCode::NO_CONTENT);
            }
            Err(_) => ApiError::not_found("Asset not found").write_to(res),
        },
        Some(_) | None => ApiError::not_found("Asset not found").write_to(res),
    }
}

fn parse_uuid_param(req: &Request, name: &str) -> Option<Uuid> {
    let value: String = req.param(name)?;
    value.parse().ok()
}
