use crate::config::GatewayConfig;
use crate::db::Database;
use std::sync::Arc;
use x402_facilitator::state::AppState as FacilitatorState;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<GatewayConfig>,
    pub db: Arc<Database>,
    pub http_client: reqwest::Client,
    /// Embedded facilitator state (when FACILITATOR_PRIVATE_KEY is set)
    pub facilitator: Option<Arc<FacilitatorState>>,
}

impl AppState {
    pub fn new(
        config: GatewayConfig,
        db: Database,
        facilitator: Option<Arc<FacilitatorState>>,
    ) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none()) // Prevent SSRF via redirects
            .build()
            .expect("failed to create HTTP client");

        Self {
            config: Arc::new(config),
            db: Arc::new(db),
            http_client,
            facilitator,
        }
    }
}
