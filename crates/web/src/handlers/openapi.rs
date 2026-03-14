//! OpenAPI specification endpoint.

use salvo::prelude::*;

use crate::openapi;

/// `GET /api/v1/openapi.json`
///
/// Returns the OpenAPI 3.0 specification for the Sivgo web API.
#[handler]
pub async fn openapi_spec(res: &mut Response) {
    let spec = openapi::generate_openapi_spec();
    res.headers_mut()
        .insert("content-type", "application/json".parse().unwrap());
    res.render(spec);
}
