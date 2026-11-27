//! Endpoint allowlists, runtime request guards, and transport trait seam.
use super::{ProbeError, RunError};
use crate::protocol::cursor_remote::runtime::worker::WorkerHooks;
use bytes::Bytes;
use futures::future::BoxFuture;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use url::Url;

pub(crate) struct Endpoints {
    pub(crate) poll: &'static str,
    pub(crate) me: &'static str,
    pub(crate) plan: &'static str,
    pub(crate) hard_limit: &'static str,
    pub(crate) current_period: &'static str,
    pub(crate) usage: &'static str,
    pub(crate) grants: &'static str,
    pub(crate) stripe: &'static str,
    pub(crate) teams: &'static str,
    pub(crate) models: &'static str,
    pub(crate) default_model: &'static str,
    pub(crate) server_config: &'static str,
    pub(crate) cli_models: &'static str,
    pub(crate) cli_default_model: &'static str,
    pub(crate) parameter_models: &'static str,
    pub(crate) run: &'static str,
    pub(crate) network: &'static str,
}

pub(crate) const ENDPOINTS: Endpoints = Endpoints {
    poll: "https://api2.cursor.sh/auth/poll",
    me: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetMe",
    plan: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetPlanInfo",
    hard_limit: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetHardLimit",
    current_period: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage",
    usage: "https://cursor.com/api/usage-summary",
    grants: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetCreditGrantsBalance",
    stripe: "https://cursor.com/api/auth/stripe",
    teams: "https://api2.cursor.sh/aiserver.v1.DashboardService/GetTeams",
    models: "https://api2.cursor.sh/agent.v1.AgentService/GetUsableModels",
    default_model: "https://api2.cursor.sh/agent.v1.AgentService/GetDefaultModelForCli",
    server_config: "https://api2.cursor.sh/aiserver.v1.ServerConfigService/GetServerConfig",
    cli_models: "https://api2.cursor.sh/aiserver.v1.AiService/GetUsableModels",
    cli_default_model: "https://api2.cursor.sh/aiserver.v1.AiService/GetDefaultModelForCli",
    parameter_models: "https://api2.cursor.sh/aiserver.v1.AiService/AvailableModels",
    run: "https://agentn.global.api5.cursor.sh/agent.v1.AgentService/Run",
    network: "https://api2.cursor.sh/",
};

const ALL_ENDPOINTS: [&str; 17] = [
    ENDPOINTS.poll,
    ENDPOINTS.me,
    ENDPOINTS.plan,
    ENDPOINTS.hard_limit,
    ENDPOINTS.current_period,
    ENDPOINTS.usage,
    ENDPOINTS.grants,
    ENDPOINTS.stripe,
    ENDPOINTS.teams,
    ENDPOINTS.models,
    ENDPOINTS.default_model,
    ENDPOINTS.server_config,
    ENDPOINTS.cli_models,
    ENDPOINTS.cli_default_model,
    ENDPOINTS.parameter_models,
    ENDPOINTS.run,
    ENDPOINTS.network,
];

const RUNTIME_READS: [&str; 12] = [
    ENDPOINTS.me,
    ENDPOINTS.usage,
    ENDPOINTS.grants,
    ENDPOINTS.stripe,
    ENDPOINTS.teams,
    ENDPOINTS.hard_limit,
    ENDPOINTS.models,
    ENDPOINTS.default_model,
    ENDPOINTS.server_config,
    ENDPOINTS.cli_models,
    ENDPOINTS.cli_default_model,
    ENDPOINTS.parameter_models,
];

pub(crate) fn validate_target(target: &str) -> Result<Url, ProbeError> {
    let url = Url::parse(target).map_err(|_| ProbeError::new("invalid_target"))?;
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(ProbeError::new("target_not_allowed"));
    }
    let origin_path = format!("{}{}", url.origin().ascii_serialization(), url.path());
    if !ALL_ENDPOINTS.contains(&origin_path.as_str()) {
        return Err(ProbeError::new("target_not_allowed"));
    }
    if url.query().is_some() {
        if origin_path != ENDPOINTS.poll {
            return Err(ProbeError::new("invalid_poll_query"));
        }
        let mut keys: Vec<String> = url.query_pairs().map(|(k, _)| k.into_owned()).collect();
        keys.sort();
        if keys != ["uuid", "verifier"] {
            return Err(ProbeError::new("invalid_poll_query"));
        }
        let uuid = url
            .query_pairs()
            .find(|(k, _)| k == "uuid")
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default();
        let verifier = url
            .query_pairs()
            .find(|(k, _)| k == "verifier")
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default();
        if uuid.len() != 36
            || !uuid
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'-'))
            || verifier.len() != 43
            || !verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(ProbeError::new("invalid_poll_query"));
        }
    }
    Ok(url)
}

pub(crate) struct UnaryRequest<'a> {
    pub(crate) method: &'static str,
    pub(crate) url: &'a str,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Bytes,
    pub(crate) timeout: Duration,
}

pub(crate) struct UnaryResponse {
    pub(crate) status: u16,
    pub(crate) content_type: String,
    pub(crate) body: Vec<u8>,
}

pub(crate) struct DuplexRequest<'a> {
    pub(crate) url: &'a str,
    pub(crate) headers: &'a [(&'a str, &'a str)],
    pub(crate) body: Bytes,
    pub(crate) timeout: Duration,
}

pub(crate) trait DuplexStream: Send {
    fn status(&self) -> u16;
    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Bytes>, ProbeError>>;
    fn send_frame(&mut self, frame: Bytes) -> Result<(), ProbeError>;
}

pub(crate) trait Transport: Send + Sync {
    fn unary<'a>(
        &'a self,
        proxy_port: u16,
        req: UnaryRequest<'a>,
    ) -> BoxFuture<'a, Result<UnaryResponse, ProbeError>>;

    fn open_duplex<'a>(
        &'a self,
        proxy_port: u16,
        req: DuplexRequest<'a>,
        hooks: &'a mut WorkerHooks,
    ) -> BoxFuture<'a, Result<Box<dyn DuplexStream>, RunError>>;
}

pub(crate) struct CheckedClient<'a> {
    proxy_port: u16,
    transport: &'a dyn Transport,
    attempted_run: AtomicBool,
}

impl<'a> CheckedClient<'a> {
    pub(crate) fn new(proxy_port: u16, transport: &'a dyn Transport) -> Self {
        Self {
            proxy_port,
            transport,
            attempted_run: AtomicBool::new(false),
        }
    }

    fn runtime_headers(extra: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut out = Vec::with_capacity(extra.len() + 4);
        for &(k, v) in extra {
            out.push((k.to_owned(), v.to_owned()));
        }
        out.push(("x-ghost-mode".into(), "true".into()));
        out.push(("x-request-id".into(), uuid::Uuid::new_v4().to_string()));
        out.push(("x-cursor-client-type".into(), "cli".into()));
        out.push((
            "x-cursor-client-version".into(),
            "cli-2026.11.22-e9042b2".into(),
        ));
        out
    }

    fn check_read_allowed(&self, url: &str) -> Result<(), ProbeError> {
        if !RUNTIME_READS.contains(&url) {
            return Err(ProbeError::new("operation_not_allowed"));
        }
        Ok(())
    }

    pub(crate) async fn post_connect_json(
        &self,
        url: &str,
        token: &str,
        body: &str,
    ) -> Result<UnaryResponse, ProbeError> {
        self.check_read_allowed(url)?;
        let auth = format!("Bearer {token}");
        let headers = Self::runtime_headers(&[
            ("accept", "application/json"),
            ("content-type", "application/json"),
            ("connect-protocol-version", "1"),
            ("authorization", &auth),
        ]);
        self.transport
            .unary(
                self.proxy_port,
                UnaryRequest {
                    method: "POST",
                    url,
                    headers,
                    body: Bytes::copy_from_slice(body.as_bytes()),
                    timeout: Duration::from_secs(15),
                },
            )
            .await
    }

    pub(crate) async fn post_connect_proto(
        &self,
        url: &str,
        token: &str,
        body: &[u8],
    ) -> Result<UnaryResponse, ProbeError> {
        self.check_read_allowed(url)?;
        let auth = format!("Bearer {token}");
        let headers = Self::runtime_headers(&[
            ("accept", "application/proto"),
            ("content-type", "application/proto"),
            ("connect-protocol-version", "1"),
            ("authorization", &auth),
        ]);
        self.transport
            .unary(
                self.proxy_port,
                UnaryRequest {
                    method: "POST",
                    url,
                    headers,
                    body: Bytes::copy_from_slice(body),
                    timeout: Duration::from_secs(15),
                },
            )
            .await
    }

    pub(crate) async fn get_cookie_json(
        &self,
        url: &str,
        cookie: &str,
    ) -> Result<UnaryResponse, ProbeError> {
        self.check_read_allowed(url)?;
        let headers = Self::runtime_headers(&[("accept", "application/json"), ("cookie", cookie)]);
        self.transport
            .unary(
                self.proxy_port,
                UnaryRequest {
                    method: "GET",
                    url,
                    headers,
                    body: Bytes::new(),
                    timeout: Duration::from_secs(15),
                },
            )
            .await
    }

    pub(crate) async fn open_duplex(
        &self,
        req: DuplexRequest<'_>,
        hooks: &mut WorkerHooks,
    ) -> Result<Box<dyn DuplexStream>, RunError> {
        if req.url != ENDPOINTS.run {
            return Err(ProbeError::new("operation_not_allowed").into());
        }
        if self.attempted_run.swap(true, Ordering::SeqCst) {
            return Err(ProbeError::new("run_already_attempted").into());
        }
        let merged = Self::runtime_headers(req.headers);
        let refs: Vec<(&str, &str)> = merged
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        self.transport
            .open_duplex(
                self.proxy_port,
                DuplexRequest {
                    url: req.url,
                    headers: &refs,
                    body: req.body,
                    timeout: req.timeout,
                },
                hooks,
            )
            .await
    }
}
