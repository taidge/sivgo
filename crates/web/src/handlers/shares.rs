//! Document sharing handlers.
//!
//! Owners can create and list share records for their documents. A share with
//! `shared_with: None` is a public link (anyone holding the token); a share
//! with `shared_with: Some(user)` is a direct share to that user only.

use chrono::Utc;
use salvo::prelude::*;
use uuid::Uuid;

use crate::audit::{self, AuditAction};
use crate::auth;
use crate::errors::ApiError;
use crate::models::{CreateShareRequest, SharePermission};
use crate::store::Store;

/// `POST /api/v1/documents/{id}/shares`
///
/// Creates a share record for a document. Only the document owner may share.
#[handler]
pub async fn create_share(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let (store, user_id) = match owner_context(req, depot, res).await {
        Some(ctx) => ctx,
        None => return,
    };
    let doc_id = match parse_uuid_param(req, "id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid document id").write_to(res);
            return;
        }
    };

    // Ownership check.
    match store.get_document(doc_id) {
        Some(doc) if doc.owner_id == user_id => {}
        _ => {
            ApiError::not_found("Document not found").write_to(res);
            return;
        }
    }

    let body: CreateShareRequest = match req.parse_json().await {
        Ok(b) => b,
        Err(e) => {
            ApiError::bad_request(format!("Invalid request body: {e}")).write_to(res);
            return;
        }
    };

    if body
        .expires_at
        .is_some_and(|expires_at| expires_at <= Utc::now())
    {
        ApiError::bad_request("expires_at must be in the future").write_to(res);
        return;
    }

    let share = store.create_share(
        doc_id,
        user_id,
        body.shared_with,
        body.permission,
        body.expires_at,
    );
    audit::record_if_available(
        depot,
        AuditAction::ShareCreated,
        Some(user_id),
        Some("share"),
        Some(share.id.to_string()),
        serde_json::json!({
            "document_id": doc_id,
            "shared_with": share.shared_with,
            "permission": format!("{:?}", share.permission),
            "expires_at": share.expires_at,
        }),
    );
    res.status_code(StatusCode::CREATED);
    res.render(Json(share));
}

/// `GET /api/v1/documents/{id}/shares`
///
/// Lists all shares for a document. Only the document owner may list.
#[handler]
pub async fn list_shares(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let (store, user_id) = match owner_context(req, depot, res).await {
        Some(ctx) => ctx,
        None => return,
    };
    let doc_id = match parse_uuid_param(req, "id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid document id").write_to(res);
            return;
        }
    };

    match store.get_document(doc_id) {
        Some(doc) if doc.owner_id == user_id => {}
        _ => {
            ApiError::not_found("Document not found").write_to(res);
            return;
        }
    }

    res.render(Json(store.get_shares(doc_id)));
}

/// `DELETE /api/v1/documents/{id}/shares/{share_id}`
///
/// Revokes a share while keeping its audit record. Only the document owner may
/// revoke shares.
#[handler]
pub async fn revoke_share(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let (store, user_id) = match owner_context(req, depot, res).await {
        Some(ctx) => ctx,
        None => return,
    };
    let doc_id = match parse_uuid_param(req, "id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid document id").write_to(res);
            return;
        }
    };
    let share_id = match parse_uuid_param(req, "share_id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid share id").write_to(res);
            return;
        }
    };

    match store.get_document(doc_id) {
        Some(doc) if doc.owner_id == user_id => {}
        _ => {
            ApiError::not_found("Document not found").write_to(res);
            return;
        }
    }

    match store.revoke_share(doc_id, share_id) {
        Some(share) => {
            audit::record_if_available(
                depot,
                AuditAction::ShareRevoked,
                Some(user_id),
                Some("share"),
                Some(share.id.to_string()),
                serde_json::json!({ "document_id": doc_id }),
            );
            res.render(Json(share));
        }
        None => ApiError::not_found("Share not found").write_to(res),
    }
}

/// `GET /api/v1/shared/{token}`
///
/// Resolves a share token to its document. Public-link shares (`shared_with:
/// None`) are accessible to anyone with the token; direct shares require the
/// authenticated user to match `shared_with`.
#[handler]
pub async fn get_shared_document(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
            return;
        }
    };

    let token: String = match req.param("token") {
        Some(t) => t,
        None => {
            ApiError::bad_request("Missing share token").write_to(res);
            return;
        }
    };

    let share = match store.find_active_share_by_token(&token) {
        Some(s) => s,
        None => {
            ApiError::not_found("Share not found").write_to(res);
            return;
        }
    };

    // Direct shares require the caller to be the designated user. We treat a
    // missing/invalid token as "not found" to avoid leaking share existence.
    let actor_id = if let Some(target) = share.shared_with {
        match auth::extract_user_id(req, depot) {
            Some(uid) if uid == target => Some(uid),
            _ => {
                ApiError::not_found("Share not found").write_to(res);
                return;
            }
        }
    } else {
        None
    };

    match store.get_document(share.document_id) {
        Some(doc) => {
            audit::record_if_available(
                depot,
                AuditAction::SharedDocumentAccessed,
                actor_id,
                Some("share"),
                Some(share.id.to_string()),
                serde_json::json!({
                    "document_id": share.document_id,
                    "permission": format!("{:?}", share.permission),
                }),
            );
            res.render(Json(SharedDocumentResponse {
                permission: share.permission,
                document: doc,
            }));
        }
        None => ApiError::not_found("Document not found").write_to(res),
    }
}

/// Response body for [`get_shared_document`].
#[derive(serde::Serialize)]
struct SharedDocumentResponse {
    permission: SharePermission,
    document: crate::models::Document,
}

/// Shared owner-auth boilerplate: resolve the store and authenticated user.
async fn owner_context(
    _req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Option<(Store, Uuid)> {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
            return None;
        }
    };
    let user_id = match auth::get_user_id(depot) {
        Some(id) => id,
        None => {
            ApiError::unauthorized("Not authenticated").write_to(res);
            return None;
        }
    };
    Some((store, user_id))
}

fn parse_uuid_param(req: &Request, name: &str) -> Option<Uuid> {
    let value: String = req.param(name)?;
    value.parse().ok()
}
