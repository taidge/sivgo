//! Authenticated preview-only access to the server's configured AI provider.

use std::sync::Arc;
use std::time::Duration;

use salvo::prelude::*;
use sivgo_agent::{AgentClient, AgentError, EditRequest, DEFAULT_BASE_URL};
use tokio::sync::Semaphore;

use crate::{errors::ApiError, rate_limit::RateLimiter};

#[derive(Clone)]
pub(crate) struct AgentService {
    client: AgentClient,
    budget: RateLimiter,
    slots: Arc<Semaphore>,
}

impl AgentService {
    pub(crate) fn from_env() -> Option<Self> {
        if std::env::var("SIVGO_AI_ENABLE_API").as_deref() != Ok("1") {
            return None;
        }
        let base = std::env::var("SIVGO_AI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());
        let model = std::env::var("SIVGO_AI_MODEL").unwrap_or_default();
        let key = std::env::var("SIVGO_AI_API_KEY").unwrap_or_default();
        match AgentClient::new(&base, &model, &key) {
            Ok(client) => Some(Self {
                client,
                budget: RateLimiter::new(10, Duration::from_secs(60)),
                slots: Arc::new(Semaphore::new(2)),
            }),
            Err(error) => {
                log::warn!("AI endpoint disabled: {error}");
                None
            }
        }
    }
}

/// POST /api/v1/agent/suggest. This endpoint does not write to the document store.
#[handler]
pub async fn suggest(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let Some(agent) = depot
        .obtain::<Option<AgentService>>()
        .ok()
        .and_then(Option::as_ref)
    else {
        ApiError {
            status: 503,
            error: "AI editing is not configured on this server.".into(),
        }
        .write_to(res);
        return;
    };
    let Ok(_slot) = agent.slots.try_acquire() else {
        ApiError::rate_limited("The AI editor is busy. Try again shortly.").write_to(res);
        return;
    };
    let Ok(request) = req.parse_json::<EditRequest>().await else {
        ApiError::bad_request("Send an SVG drawing, an instruction and optional chat context.")
            .write_to(res);
        return;
    };
    if !agent.budget.allow("agent") {
        ApiError::rate_limited("The AI request limit was reached. Try again shortly.")
            .write_to(res);
        return;
    }
    match agent.client.suggest(&request).await {
        Ok(proposal) => res.render(Json(proposal)),
        Err(error) => {
            let status = match error {
                AgentError::Input(_) => 400,
                AgentError::Timeout => 504,
                _ => 502,
            };
            ApiError {
                status,
                error: error.to_string(),
            }
            .write_to(res);
        }
    }
}
