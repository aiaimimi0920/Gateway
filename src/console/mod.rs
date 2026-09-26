pub mod config;
pub use config::*;

pub mod auth;
pub mod document;
pub mod gemini_auth_sessions;
pub mod journal;
pub mod persistence;
pub mod redis_store;
pub mod revision;
pub mod runtime;
pub mod secrets;

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
    RouteConfigRedisStore, RouteConfigRedisStoreError,
};
pub use runtime::{
    PooledRouteConfigRedisBackend, RouteConfigCoordinator, RouteConfigRedisBackend,
    RouteConfigReplica, RouteConfigRuntime, RouteConfigRuntimeError,
};

#[cfg(test)]
mod persistence_lock_contract;
