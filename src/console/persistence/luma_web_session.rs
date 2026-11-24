//! Secret runtime overlays use the same protected files, writer lock and atomic replacement as credentials.
use super::{file_io::read_json_limited, paths::ensure_secure_directory, *};

impl RouteConfigPersistence {
    fn luma_web_session_path(&self, key: &str) -> Result<PathBuf, PersistenceError> {
        if key.len() != 64 || !key.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(PersistenceError::corruption(
                "Invalid Luma Web session identity",
            ));
        }
        let dir = self.state_root().join("luma-web-sessions");
        super::paths::reject_existing_links(&dir)?;
        let path = dir.join(format!("{key}.json"));
        self.validate_optional_managed_file(&path)?;
        Ok(path)
    }
    pub(crate) fn read_luma_web_session(
        &self,
        key: &str,
    ) -> Result<Option<String>, PersistenceError> {
        let _guard = self.try_writer_lock()?;
        self.load_luma_web_session(key)
    }
    fn load_luma_web_session(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        let path = self.luma_web_session_path(key)?;
        if !path
            .try_exists()
            .map_err(|e| PersistenceError::io("reading Luma Web session", e))?
        {
            return Ok(None);
        }
        let raw: String = read_json_limited(&path)?;
        if raw.len() > 65536 {
            return Err(PersistenceError::corruption("Oversized Luma Web session"));
        }
        Ok(Some(raw))
    }
    pub(crate) fn cas_luma_web_session(
        &self,
        key: &str,
        expected: Option<&str>,
        next: &str,
    ) -> Result<bool, PersistenceError> {
        if next.len() > 65536 {
            return Err(PersistenceError::corruption("Oversized Luma Web session"));
        }
        let guard = self.try_writer_lock()?;
        self.ensure_writable()?;
        let current = self.load_luma_web_session(key)?;
        if current.as_deref() == Some(next) {
            return Ok(true);
        }
        if current.as_deref() != expected {
            return Ok(false);
        }
        let path = self.luma_web_session_path(key)?;
        ensure_secure_directory(path.parent().expect("session directory"))?;
        self.atomic_write_json_locked(&guard, &path, &next)?;
        Ok(true)
    }
}
