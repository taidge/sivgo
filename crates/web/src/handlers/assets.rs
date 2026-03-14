//! Asset upload/download handlers.

use salvo::prelude::*;

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

    let assets = store.list(&user_id.to_string());
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

    // For simplicity, store the base64 string as raw bytes.
    // In production, decode the base64 first.
    let data = body.data.into_bytes();
    let id = store.upload(
        &user_id.to_string(),
        &body.filename,
        &body.content_type,
        data,
    );

    res.status_code(StatusCode::CREATED);
    res.render(Json(serde_json::json!({ "id": id })));
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

    let asset_id: String = match req.param("id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Missing asset id").write_to(res);
            return;
        }
    };

    match store.download(&asset_id) {
        Some(asset) => {
            res.headers_mut().insert(
                "content-type",
                asset
                    .content_type
                    .parse()
                    .unwrap_or_else(|_| "application/octet-stream".parse().unwrap()),
            );
            res.headers_mut().insert(
                "content-disposition",
                format!("attachment; filename=\"{}\"", asset.filename)
                    .parse()
                    .unwrap(),
            );
            res.body(asset.data);
        }
        None => {
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

    let asset_id: String = match req.param("id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Missing asset id").write_to(res);
            return;
        }
    };

    if store.delete(&asset_id) {
        res.status_code(StatusCode::NO_CONTENT);
    } else {
        ApiError::not_found("Asset not found").write_to(res);
    }
}
