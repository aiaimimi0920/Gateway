//! 单个 owned child 只准入一个会话；账号选择／代次校验与 HTTP 生命周期由上层负责。
use crate::RuntimeConfig;
use github_copilot_sdk::{
    handler::DenyAllHandler, session::PreparedSession, Client, SessionConfig,
};
use std::sync::Arc;

/// 内部错误保留 SDK 原因；跨 API／日志边界须使用 Gateway 的秘密脱敏策略。
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("invalid Copilot runtime configuration")]
    Configuration(#[source] std::io::Error),
    #[error("Copilot runtime startup failed")]
    Start(#[source] github_copilot_sdk::Error),
    #[error("Copilot runtime already reserved a session")]
    SessionReserved,
    #[error("Copilot token and model must not be empty")]
    EmptyIdentity,
    #[error("Copilot catalog token must not be empty")]
    EmptyCatalogToken,
    #[error("Copilot model catalog request failed")]
    Catalog(#[source] github_copilot_sdk::Error),
    #[error("Copilot session preparation failed")]
    Session(#[source] github_copilot_sdk::Error),
    #[error("Copilot runtime shutdown failed")]
    Stop(#[source] github_copilot_sdk::StopErrors),
}

pub struct CopilotRuntime {
    client: Client,
    session_reserved: bool,
}

impl CopilotRuntime {
    pub async fn start(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let options = config.options().map_err(RuntimeError::Configuration)?;
        let client = Client::start(options).await.map_err(RuntimeError::Start)?;
        Ok(Self {
            client,
            session_reserved: false,
        })
    }

    /// 返回未启动会话，使调用者先 subscribe 再 start，避免丢失早到事件。
    /// token 必须由上层明确选择；不承担授权／refresh，也不暴露 session resume 入口。
    pub fn prepare_text_session(
        &mut self,
        token: String,
        model: String,
    ) -> Result<PreparedSession, RuntimeError> {
        if self.session_reserved {
            return Err(RuntimeError::SessionReserved);
        }
        if token.trim().is_empty() || model.trim().is_empty() {
            return Err(RuntimeError::EmptyIdentity);
        }
        let mut config = SessionConfig::default()
            .with_github_token(token)
            .with_model(model)
            .with_available_tools(Vec::<String>::new())
            .with_permission_handler(Arc::new(DenyAllHandler));
        config.included_builtin_agents = Some(Vec::new());
        let prepared = self
            .client
            .prepare_session(config)
            .map_err(RuntimeError::Session)?;
        // 一旦准入，不复用可能已发送／取消的上下文，也不隐藏重放。
        self.session_reserved = true;
        Ok(prepared)
    }

    pub async fn stop(self) -> Result<(), RuntimeError> {
        self.client.stop().await.map_err(RuntimeError::Stop)
    }

    /// 仅供受信任调用方：token 来自已选择且校验代次的账号，不读取进程账号。
    /// 固定 SDK 的 experimental typed RPC 绕过全客户端目录缓存；保留 policy/billing 等
    /// 原始类型信息，不将未知费用补零，也不把目录结果当作推理授权。调用方拥有超时／取消。
    pub async fn list_models_for_token(
        &self,
        token: String,
    ) -> Result<Vec<github_copilot_sdk::Model>, RuntimeError> {
        if token.trim().is_empty() {
            return Err(RuntimeError::EmptyCatalogToken);
        }
        self.client
            .rpc()
            .models()
            .list_with_params(github_copilot_sdk::rpc::ModelsListRequest {
                git_hub_token: Some(token),
                ..Default::default()
            })
            .await
            .map(|result| result.models)
            .map_err(RuntimeError::Catalog)
    }
}

impl Drop for CopilotRuntime {
    fn drop(&mut self) {
        // stop 被取消或 owner 提前释放时，不能让持有 Session 的任务延长 child 生命周期。
        // 进程树关闭仍交由 SDK 实现，不另建竞争 kill/reap owner。
        self.client.force_stop();
    }
}
