//! CRUD handlers for SVG documents.

use salvo::prelude::*;
use uuid::Uuid;

use crate::auth;
use crate::errors::ApiError;
use crate::models::*;
use crate::store::Store;

/// Maximum accepted size of a document's SVG body (bounds in-memory storage).
const MAX_DOCUMENT_SVG_BYTES: usize = 16 * 1024 * 1024;

/// `GET /api/v1/documents`
///
/// Lists all documents owned by the authenticated user.
#[handler]
pub async fn list_documents(depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
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

    let docs = store.list_documents(user_id);
    res.render(Json(docs));
}

/// `POST /api/v1/documents`
///
/// Creates a new document. The request body should contain `title` and
/// optionally `svg_content`.
#[handler]
pub async fn create_document(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
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

    let body: CreateDocumentRequest = match req.parse_json().await {
        Ok(b) => b,
        Err(e) => {
            ApiError::bad_request(format!("Invalid request body: {e}")).write_to(res);
            return;
        }
    };

    if body.title.is_empty() {
        ApiError::bad_request("title is required").write_to(res);
        return;
    }

    if let Some(svg) = body.svg_content.as_deref() {
        if svg.len() > MAX_DOCUMENT_SVG_BYTES {
            ApiError::bad_request("svg_content exceeds 16 MiB limit").write_to(res);
            return;
        }
    }

    let default_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600"></svg>"#;
    let svg_content = body.svg_content.as_deref().unwrap_or(default_svg);

    let doc = store.create_document(user_id, &body.title, svg_content);

    // Save initial version.
    store.save_version(doc.id, svg_content);

    res.status_code(StatusCode::CREATED);
    res.render(Json(doc));
}

/// `GET /api/v1/documents/{id}`
///
/// Retrieves a single document by id.
#[handler]
pub async fn get_document(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
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

    let doc_id = match parse_uuid_param(req, "id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid document id").write_to(res);
            return;
        }
    };

    match store.get_document(doc_id) {
        Some(doc) if doc.owner_id == user_id => {
            res.render(Json(doc));
        }
        Some(_) => {
            ApiError::not_found("Document not found").write_to(res);
        }
        None => {
            ApiError::not_found("Document not found").write_to(res);
        }
    }
}

/// `PUT /api/v1/documents/{id}`
///
/// Updates a document's title and/or svg_content.
#[handler]
pub async fn update_document(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
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

    let doc_id = match parse_uuid_param(req, "id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid document id").write_to(res);
            return;
        }
    };

    // Verify ownership.
    match store.get_document(doc_id) {
        Some(doc) if doc.owner_id == user_id => {}
        _ => {
            ApiError::not_found("Document not found").write_to(res);
            return;
        }
    }

    let body: UpdateDocumentRequest = match req.parse_json().await {
        Ok(b) => b,
        Err(e) => {
            ApiError::bad_request(format!("Invalid request body: {e}")).write_to(res);
            return;
        }
    };

    // Save a version before updating if svg_content changed.
    if let Some(ref content) = body.svg_content {
        if content.len() > MAX_DOCUMENT_SVG_BYTES {
            ApiError::bad_request("svg_content exceeds 16 MiB limit").write_to(res);
            return;
        }
        store.save_version(doc_id, content);
    }

    match store.update_document(doc_id, body.title.as_deref(), body.svg_content.as_deref()) {
        Some(doc) => res.render(Json(doc)),
        None => ApiError::not_found("Document not found").write_to(res),
    }
}

/// `DELETE /api/v1/documents/{id}`
///
/// Deletes a document.
#[handler]
pub async fn delete_document(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
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

    let doc_id = match parse_uuid_param(req, "id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid document id").write_to(res);
            return;
        }
    };

    // Verify ownership.
    match store.get_document(doc_id) {
        Some(doc) if doc.owner_id == user_id => {}
        _ => {
            ApiError::not_found("Document not found").write_to(res);
            return;
        }
    }

    store.delete_document(doc_id);
    res.status_code(StatusCode::NO_CONTENT);
}

/// `GET /api/v1/documents/{id}/versions`
///
/// Lists all saved versions of a document.
#[handler]
pub async fn list_versions(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
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

    let doc_id = match parse_uuid_param(req, "id") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Invalid document id").write_to(res);
            return;
        }
    };

    // Verify ownership.
    match store.get_document(doc_id) {
        Some(doc) if doc.owner_id == user_id => {}
        _ => {
            ApiError::not_found("Document not found").write_to(res);
            return;
        }
    }

    let versions = store.get_versions(doc_id);
    res.render(Json(versions));
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a UUID path parameter by name.
fn parse_uuid_param(req: &Request, name: &str) -> Option<Uuid> {
    let value: String = req.param(name)?;
    value.parse().ok()
}
