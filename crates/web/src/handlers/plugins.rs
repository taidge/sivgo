//! Plugin registry handlers.

use salvo::prelude::*;

use crate::auth;
use crate::errors::ApiError;
use crate::plugin_registry::PluginRegistry;

/// `GET /api/v1/plugins`
///
/// Lists all available plugins in the catalogue.
#[handler]
pub async fn list_plugins(depot: &mut Depot, res: &mut Response) {
    let registry = match depot.obtain::<PluginRegistry>() {
        Ok(r) => r.clone(),
        Err(_) => {
            ApiError::internal("PluginRegistry unavailable").write_to(res);
            return;
        }
    };

    let plugins = registry.list();
    res.render(Json(plugins));
}

/// `GET /api/v1/plugins/installed`
///
/// Lists plugins installed by the authenticated user.
#[handler]
pub async fn list_installed_plugins(depot: &mut Depot, res: &mut Response) {
    let registry = match depot.obtain::<PluginRegistry>() {
        Ok(r) => r.clone(),
        Err(_) => {
            ApiError::internal("PluginRegistry unavailable").write_to(res);
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

    let plugins = registry.user_plugins(&user_id.to_string());
    res.render(Json(plugins));
}

/// `POST /api/v1/plugins/{pluginId}/install`
///
/// Installs a plugin for the authenticated user.
#[handler]
pub async fn install_plugin(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let registry = match depot.obtain::<PluginRegistry>() {
        Ok(r) => r.clone(),
        Err(_) => {
            ApiError::internal("PluginRegistry unavailable").write_to(res);
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

    let plugin_id: String = match req.param("pluginId") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Missing pluginId parameter").write_to(res);
            return;
        }
    };

    if registry.install(&user_id.to_string(), &plugin_id) {
        res.render(Json(serde_json::json!({
            "status": "installed",
            "plugin_id": plugin_id,
        })));
    } else {
        ApiError::conflict(format!("Plugin {plugin_id} not found or already installed"))
            .write_to(res);
    }
}

/// `POST /api/v1/plugins/{pluginId}/uninstall`
///
/// Uninstalls a plugin for the authenticated user.
#[handler]
pub async fn uninstall_plugin(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let registry = match depot.obtain::<PluginRegistry>() {
        Ok(r) => r.clone(),
        Err(_) => {
            ApiError::internal("PluginRegistry unavailable").write_to(res);
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

    let plugin_id: String = match req.param("pluginId") {
        Some(id) => id,
        None => {
            ApiError::bad_request("Missing pluginId parameter").write_to(res);
            return;
        }
    };

    if registry.uninstall(&user_id.to_string(), &plugin_id) {
        res.render(Json(serde_json::json!({
            "status": "uninstalled",
            "plugin_id": plugin_id,
        })));
    } else {
        ApiError::not_found(format!("Plugin {plugin_id} is not installed")).write_to(res);
    }
}
