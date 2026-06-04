//! OpenAPI 3.0 documentation generator.
//!
//! Produces a JSON string describing all Sivgo web API endpoints following
//! the OpenAPI 3.0 specification.

/// Generate an OpenAPI 3.0 specification (as a JSON string) that documents
/// all the Sivgo web API endpoints.
pub fn generate_openapi_spec() -> String {
    let spec = serde_json::json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Sivgo Web API",
            "description": "REST API for the Sivgo online SVG editor. Supports user authentication, document management, asset upload/download, user preferences sync, and plugin management.",
            "version": "0.1.0",
            "contact": {
                "name": "Sivgo Team"
            }
        },
        "servers": [
            {
                "url": "http://localhost:3000",
                "description": "Local development server"
            }
        ],
        "paths": {
            "/api/v1/health": {
                "get": {
                    "summary": "Health check",
                    "operationId": "healthCheck",
                    "tags": ["system"],
                    "responses": {
                        "200": {
                            "description": "Server is healthy",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "status": { "type": "string", "example": "ok" },
                                            "service": { "type": "string", "example": "sivgo-server" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/auth/register": {
                "post": {
                    "summary": "Register a new user",
                    "operationId": "register",
                    "tags": ["auth"],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/RegisterRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "User created",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                                }
                            }
                        },
                        "400": { "description": "Invalid request body" },
                        "409": { "description": "Email already registered" }
                    }
                }
            },
            "/api/v1/auth/login": {
                "post": {
                    "summary": "Log in with email and password",
                    "operationId": "login",
                    "tags": ["auth"],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/LoginRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Login successful",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                                }
                            }
                        },
                        "401": { "description": "Invalid credentials" }
                    }
                }
            },
            "/api/v1/auth/oauth/{provider}": {
                "get": {
                    "summary": "Initiate OAuth2 login flow",
                    "operationId": "oauthLogin",
                    "tags": ["auth"],
                    "parameters": [
                        {
                            "name": "provider",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "enum": ["google", "github"] }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "OAuth authorization URL",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OAuthLoginResponse" }
                                }
                            }
                        },
                        "400": { "description": "Unsupported provider" }
                    }
                }
            },
            "/api/v1/auth/oauth/callback": {
                "get": {
                    "summary": "OAuth2 callback endpoint",
                    "operationId": "oauthCallback",
                    "tags": ["auth"],
                    "parameters": [
                        {
                            "name": "code",
                            "in": "query",
                            "required": true,
                            "schema": { "type": "string" }
                        },
                        {
                            "name": "state",
                            "in": "query",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "OAuth login successful",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OAuthCallbackResponse" }
                                }
                            }
                        },
                        "400": { "description": "Invalid code or state" }
                    }
                }
            },
            "/api/v1/documents": {
                "get": {
                    "summary": "List documents owned by the authenticated user",
                    "operationId": "listDocuments",
                    "tags": ["documents"],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "List of documents",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/Document" }
                                    }
                                }
                            }
                        },
                        "401": { "description": "Unauthorized" }
                    }
                },
                "post": {
                    "summary": "Create a new document",
                    "operationId": "createDocument",
                    "tags": ["documents"],
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateDocumentRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Document created",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/Document" }
                                }
                            }
                        },
                        "400": { "description": "Invalid request" },
                        "401": { "description": "Unauthorized" }
                    }
                }
            },
            "/api/v1/documents/{id}": {
                "get": {
                    "summary": "Get a document by id",
                    "operationId": "getDocument",
                    "tags": ["documents"],
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Document found",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/Document" }
                                }
                            }
                        },
                        "404": { "description": "Not found" }
                    }
                },
                "put": {
                    "summary": "Update a document",
                    "operationId": "updateDocument",
                    "tags": ["documents"],
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/UpdateDocumentRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": { "description": "Document updated" },
                        "404": { "description": "Not found" }
                    }
                },
                "delete": {
                    "summary": "Delete a document",
                    "operationId": "deleteDocument",
                    "tags": ["documents"],
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" }
                        }
                    ],
                    "responses": {
                        "204": { "description": "Document deleted" },
                        "404": { "description": "Not found" }
                    }
                }
            },
            "/api/v1/documents/{id}/versions": {
                "get": {
                    "summary": "List versions of a document",
                    "operationId": "listVersions",
                    "tags": ["documents"],
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "List of versions",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/DocumentVersion" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/export": {
                "post": {
                    "summary": "Export SVG to PNG or normalized SVG",
                    "operationId": "exportDocument",
                    "tags": ["export"],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/ExportRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Exported file",
                            "content": {
                                "image/png": {
                                    "schema": { "type": "string", "format": "binary" }
                                },
                                "image/svg+xml": {
                                    "schema": { "type": "string" }
                                }
                            }
                        },
                        "400": { "description": "Invalid SVG or parameters" }
                    }
                }
            },
            "/api/v1/assets": {
                "get": {
                    "summary": "List assets owned by the authenticated user",
                    "operationId": "listAssets",
                    "tags": ["assets"],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "List of assets",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/Asset" }
                                    }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "summary": "Upload a new asset",
                    "operationId": "uploadAsset",
                    "tags": ["assets"],
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/UploadAssetRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Asset uploaded",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/Asset" }
                                }
                            }
                        },
                        "400": { "description": "Invalid request" }
                    }
                }
            },
            "/api/v1/assets/{id}": {
                "get": {
                    "summary": "Download an asset by id",
                    "operationId": "downloadAsset",
                    "tags": ["assets"],
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Asset data",
                            "content": {
                                "application/octet-stream": {
                                    "schema": { "type": "string", "format": "binary" }
                                }
                            }
                        },
                        "404": { "description": "Not found" }
                    }
                },
                "delete": {
                    "summary": "Delete an asset",
                    "operationId": "deleteAsset",
                    "tags": ["assets"],
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" }
                        }
                    ],
                    "responses": {
                        "204": { "description": "Asset deleted" },
                        "404": { "description": "Not found" }
                    }
                }
            },
            "/api/v1/preferences": {
                "get": {
                    "summary": "Get user preferences",
                    "operationId": "getPreferences",
                    "tags": ["preferences"],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "User preferences",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/UserPreferences" }
                                }
                            }
                        }
                    }
                },
                "put": {
                    "summary": "Replace user preferences",
                    "operationId": "setPreferences",
                    "tags": ["preferences"],
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/UserPreferences" }
                            }
                        }
                    },
                    "responses": {
                        "200": { "description": "Preferences updated" }
                    }
                },
                "patch": {
                    "summary": "Partially update user preferences",
                    "operationId": "mergePreferences",
                    "tags": ["preferences"],
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "description": "Partial UserPreferences object"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": { "description": "Preferences merged" }
                    }
                }
            },
            "/api/v1/plugins": {
                "get": {
                    "summary": "List all available plugins",
                    "operationId": "listPlugins",
                    "tags": ["plugins"],
                    "responses": {
                        "200": {
                            "description": "Plugin catalogue",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/PluginInfo" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/plugins/installed": {
                "get": {
                    "summary": "List plugins installed by the authenticated user",
                    "operationId": "listInstalledPlugins",
                    "tags": ["plugins"],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Installed plugins",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/PluginInfo" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/plugins/{pluginId}/install": {
                "post": {
                    "summary": "Install a plugin for the authenticated user",
                    "operationId": "installPlugin",
                    "tags": ["plugins"],
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "pluginId",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": { "description": "Plugin installed" },
                        "404": { "description": "Plugin not found" },
                        "409": { "description": "Already installed" }
                    }
                }
            },
            "/api/v1/plugins/{pluginId}/uninstall": {
                "post": {
                    "summary": "Uninstall a plugin for the authenticated user",
                    "operationId": "uninstallPlugin",
                    "tags": ["plugins"],
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "pluginId",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": { "description": "Plugin uninstalled" },
                        "404": { "description": "Plugin not installed" }
                    }
                }
            },
            "/api/v1/openapi.json": {
                "get": {
                    "summary": "Get OpenAPI specification",
                    "operationId": "getOpenApiSpec",
                    "tags": ["system"],
                    "responses": {
                        "200": {
                            "description": "OpenAPI 3.0 specification",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "JWT"
                }
            },
            "schemas": {
                "RegisterRequest": {
                    "type": "object",
                    "required": ["email", "username", "password"],
                    "properties": {
                        "email": { "type": "string", "format": "email" },
                        "username": { "type": "string" },
                        "password": { "type": "string", "format": "password" }
                    }
                },
                "LoginRequest": {
                    "type": "object",
                    "required": ["email", "password"],
                    "properties": {
                        "email": { "type": "string", "format": "email" },
                        "password": { "type": "string", "format": "password" }
                    }
                },
                "AuthResponse": {
                    "type": "object",
                    "required": ["token", "user"],
                    "properties": {
                        "token": { "type": "string" },
                        "user": { "$ref": "#/components/schemas/User" }
                    }
                },
                "OAuthLoginResponse": {
                    "type": "object",
                    "required": ["auth_url", "state"],
                    "properties": {
                        "auth_url": { "type": "string", "format": "uri" },
                        "state": { "type": "string" }
                    }
                },
                "OAuthCallbackResponse": {
                    "type": "object",
                    "required": ["access_token", "expires_in", "provider", "user"],
                    "properties": {
                        "access_token": { "type": "string" },
                        "refresh_token": { "type": "string", "nullable": true },
                        "expires_in": { "type": "integer" },
                        "provider": { "type": "string", "enum": ["google", "github"] },
                        "user": { "$ref": "#/components/schemas/OAuthUserInfo" }
                    }
                },
                "OAuthUserInfo": {
                    "type": "object",
                    "required": ["email", "name", "provider_user_id"],
                    "properties": {
                        "email": { "type": "string" },
                        "name": { "type": "string" },
                        "avatar_url": { "type": "string", "nullable": true },
                        "provider_user_id": { "type": "string" }
                    }
                },
                "User": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "email": { "type": "string" },
                        "username": { "type": "string" },
                        "created_at": { "type": "string", "format": "date-time" }
                    }
                },
                "Document": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "owner_id": { "type": "string", "format": "uuid" },
                        "title": { "type": "string" },
                        "svg_content": { "type": "string" },
                        "created_at": { "type": "string", "format": "date-time" },
                        "updated_at": { "type": "string", "format": "date-time" }
                    }
                },
                "DocumentVersion": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "document_id": { "type": "string", "format": "uuid" },
                        "version": { "type": "integer" },
                        "svg_content": { "type": "string" },
                        "created_at": { "type": "string", "format": "date-time" }
                    }
                },
                "CreateDocumentRequest": {
                    "type": "object",
                    "required": ["title"],
                    "properties": {
                        "title": { "type": "string" },
                        "svg_content": { "type": "string" }
                    }
                },
                "UpdateDocumentRequest": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "svg_content": { "type": "string" }
                    }
                },
                "ExportRequest": {
                    "type": "object",
                    "required": ["svg_content", "format"],
                    "properties": {
                        "svg_content": { "type": "string" },
                        "format": { "type": "string", "enum": ["Png", "Svg"] },
                        "width": { "type": "integer" },
                        "height": { "type": "integer" }
                    }
                },
                "Asset": {
                    "type": "object",
                    "required": ["id", "filename", "content_type", "size_bytes", "created_at", "url"],
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "filename": { "type": "string" },
                        "content_type": { "type": "string" },
                        "size_bytes": { "type": "integer" },
                        "created_at": { "type": "string", "format": "date-time" },
                        "url": { "type": "string" }
                    }
                },
                "UploadAssetRequest": {
                    "type": "object",
                    "required": ["filename", "content_type", "data"],
                    "properties": {
                        "filename": { "type": "string" },
                        "content_type": { "type": "string" },
                        "data": { "type": "string", "format": "byte", "description": "Base64-encoded file data" }
                    }
                },
                "UserPreferences": {
                    "type": "object",
                    "properties": {
                        "theme": { "type": "string" },
                        "default_unit": { "type": "string" },
                        "grid_visible": { "type": "boolean" },
                        "guides_visible": { "type": "boolean" },
                        "snap_enabled": { "type": "boolean" },
                        "autosave_interval": { "type": "integer" },
                        "recent_colors": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "custom_shortcuts": {
                            "type": "array",
                            "items": {
                                "type": "array",
                                "items": { "type": "string" },
                                "minItems": 2,
                                "maxItems": 2
                            }
                        }
                    }
                },
                "PluginInfo": {
                    "type": "object",
                    "required": ["id", "name", "version", "description", "author"],
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" },
                        "version": { "type": "string" },
                        "description": { "type": "string" },
                        "author": { "type": "string" }
                    }
                }
            }
        }
    });

    serde_json::to_string_pretty(&spec).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_is_valid_json() {
        let spec = generate_openapi_spec();
        let parsed: serde_json::Value =
            serde_json::from_str(&spec).expect("spec should be valid JSON");
        assert_eq!(parsed["openapi"], "3.0.3");
    }

    #[test]
    fn spec_contains_all_endpoints() {
        let spec = generate_openapi_spec();
        let parsed: serde_json::Value = serde_json::from_str(&spec).unwrap();
        let paths = parsed["paths"].as_object().expect("paths should be object");

        let expected_paths = [
            "/api/v1/health",
            "/api/v1/auth/register",
            "/api/v1/auth/login",
            "/api/v1/documents",
            "/api/v1/documents/{id}",
            "/api/v1/documents/{id}/versions",
            "/api/v1/export",
            "/api/v1/assets",
            "/api/v1/assets/{id}",
            "/api/v1/preferences",
            "/api/v1/plugins",
            "/api/v1/openapi.json",
        ];
        for path in &expected_paths {
            assert!(
                paths.contains_key(*path),
                "spec should contain path: {path}"
            );
        }
    }

    #[test]
    fn spec_has_security_scheme() {
        let spec = generate_openapi_spec();
        let parsed: serde_json::Value = serde_json::from_str(&spec).unwrap();
        let bearer = &parsed["components"]["securitySchemes"]["bearerAuth"];
        assert_eq!(bearer["type"], "http");
        assert_eq!(bearer["scheme"], "bearer");
    }
}
