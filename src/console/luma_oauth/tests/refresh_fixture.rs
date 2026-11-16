//! Local fixtures exercise the real permanent-grant and configuration transaction owners.
use super::{account, support};
use crate::{
    console::{ConsoleConfig, ConsoleConfigValues, RouteConfigRuntime},
    state::AppState,
};
use std::sync::Arc;

pub(super) struct Local {
    pub state: Arc<AppState>,
    pub config: ConsoleConfig,
}
impl Local {
    pub fn new() -> Self {
        let mut provider = account::provider();
        provider["credentials"][0]["refresh_token"] = serde_json::json!("old-refresh");
        provider["credentials"][0]["luma_oauth"]["offline_access"] = serde_json::json!(true);
        let root = std::env::temp_dir().join(format!("luma-refresh-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        std::fs::write(
            &routes,
            serde_yaml::to_string(&account::document(provider)).unwrap(),
        )
        .unwrap();
        let config = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(root.join("state")),
            routes_file: Some(routes),
            ..Default::default()
        })
        .unwrap();
        let runtime = Arc::new(RouteConfigRuntime::new_local(&config).unwrap());
        let state = support::build_state(runtime.route_config().clone(), Some(runtime));
        Self { state, config }
    }
    pub fn reopen(&self) -> Arc<AppState> {
        let runtime = Arc::new(RouteConfigRuntime::new_local(&self.config).unwrap());
        support::build_state(runtime.route_config().clone(), Some(runtime))
    }
}
