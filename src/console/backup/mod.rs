//! Password-encrypted portable backups. Plaintext never belongs in sync transport or logs.
mod archive;
mod database;
mod encryption;
mod files;
pub mod remote;
pub mod service;

pub use encryption::{decrypt, encrypt, MAX_BACKUP_BYTES};

mod sqlite_files;
#[cfg(test)]
mod tests;
