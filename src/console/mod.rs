pub mod config;
pub use config::*;

pub mod accio_oauth;
pub mod antigravity_oauth;
pub mod auth;
pub mod backup;
pub mod chatgpt_oauth;
#[cfg(feature = "copilot-sdk-runtime")]
pub(crate) mod copilot_catalog;
pub mod diagnostics;
pub mod dim_oauth;
pub mod document;
pub mod freebuff_login;
pub(crate) mod freebuff_purchase;
pub mod gemini_auth_sessions;
pub mod google_web_json;
pub mod grok_oauth;
pub mod journal;
pub mod kiro_oauth;
pub mod lobsterai_oauth;
pub mod luma_oauth;
pub mod nous_oauth;
pub mod persistence;
pub mod quick_oauth;
pub mod redis_store;
pub mod refresh_grants;
pub mod revision;
pub mod runtime;
pub mod secrets;
pub mod vertex_oauth;

pub use auth::{
    AuthenticatedConsoleActor, ConsoleAuthRuntime, ConsoleBootstrapStatus, ConsoleRequestContext,
    ConsoleSessionView, SecretGrantView,
};
pub use gemini_auth_sessions::{
    gemini_auth_session_manager, CreateGeminiAuthSessionInput, GeminiAuthFamily,
    GeminiAuthSessionView,
};
pub use journal::{
    JournalEntry, JournalError, RecoveryDisposition, RecoveryReport, TransactionJournal,
};
pub use persistence::{
    AtomicReplaceBackend, ConsolePersistenceError, FirstSavePresence, FirstSaveStatus,
    PersistenceError, PlatformAtomicReplaceBackend, RevisionArchive, RouteConfigPersistence,
    StoredRouteRevision, TransactionPhase, TransactionRecord, WriterLockGuard, YamlReplaceReceipt,
};
pub use redis_store::{
    RouteConfigRedisActivationOutcome, RouteConfigRedisKeys, RouteConfigRedisRevision,
    RouteConfigRedisSettlement, RouteConfigRedisStore, RouteConfigRedisStoreError,
};
pub use runtime::{
    PooledRouteConfigRedisBackend, RouteConfigCoordinator, RouteConfigRedisBackend,
    RouteConfigReplica, RouteConfigRuntime, RouteConfigRuntimeError,
};

#[cfg(test)]
mod persistence_lock_contract;
