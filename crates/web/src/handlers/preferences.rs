//! User preferences handlers.

use salvo::prelude::*;

use crate::auth;
use crate::errors::ApiError;
use crate::preferences::{PreferencesStore, UserPreferences};

/// `GET /api/v1/preferences`
///
/// Returns the preferences for the authenticated user, or defaults if none
/// have been saved yet.
#[handler]
pub async fn get_preferences(depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<PreferencesStore>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("PreferencesStore unavailable").write_to(res);
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

    let prefs = store.get(user_id).unwrap_or_default();
    res.render(Json(prefs));
}

/// `PUT /api/v1/preferences`
///
/// Replaces the full preferences for the authenticated user.
#[handler]
pub async fn set_preferences(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<PreferencesStore>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("PreferencesStore unavailable").write_to(res);
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

    let prefs: UserPreferences = match req.parse_json().await {
        Ok(p) => p,
        Err(e) => {
            ApiError::bad_request(format!("Invalid request body: {e}")).write_to(res);
            return;
        }
    };

    store.set(user_id, prefs.clone());
    res.render(Json(prefs));
}

/// `PATCH /api/v1/preferences`
///
/// Merges a partial JSON object into the existing preferences.
#[handler]
pub async fn merge_preferences(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<PreferencesStore>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("PreferencesStore unavailable").write_to(res);
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

    let partial: serde_json::Value = match req.parse_json().await {
        Ok(v) => v,
        Err(e) => {
            ApiError::bad_request(format!("Invalid request body: {e}")).write_to(res);
            return;
        }
    };

    store.merge(user_id, partial);

    let prefs = store.get(user_id).unwrap_or_default();
    res.render(Json(prefs));
}
