//! Metadata-only channel within one supervisor-owned browser workspace, never a profile locator API.
use std::{io, path::PathBuf};

#[derive(Clone)]
pub(crate) struct ConsoleBrowserInteraction {
    root: PathBuf,
}

impl ConsoleBrowserInteraction {
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    async fn check_root(&self) -> io::Result<()> {
        if tokio::fs::canonicalize(&self.root).await? != self.root {
            return Err(io::Error::other("browser interaction owner changed"));
        }
        Ok(())
    }

    pub(crate) async fn phase(&self) -> io::Result<&'static str> {
        self.check_root().await?;
        const PHASES: [&str; 5] = [
            "login_window",
            "session_read",
            "verification_required",
            "verification_window",
            "session_read",
        ];
        for step in (1..=PHASES.len()).rev() {
            let file = self.root.join(format!("login-step-{step}"));
            let metadata = match tokio::fs::symlink_metadata(file).await {
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                result => result?,
            };
            if !metadata.is_file() || metadata.len() != 0 {
                return Err(io::Error::other("invalid browser interaction phase"));
            }
            return Ok(PHASES[step - 1]);
        }
        Ok("login_window")
    }

    pub(crate) async fn verify(&self) -> io::Result<()> {
        if self.phase().await? != "verification_required" {
            return Err(io::Error::other("browser verification is not pending"));
        }
        // One empty marker, create-new only: repeated clicks cannot authorize another recovery.
        tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.root.join("login-verify"))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::upstream::console_browser_workspace::ConsoleBrowserWorkspace;

    #[tokio::test]
    async fn channel_allows_one_explicit_verification_only_in_the_pending_phase() {
        let workspace = ConsoleBrowserWorkspace::create().await.unwrap();
        let channel = workspace.interaction();
        assert_eq!(channel.phase().await.unwrap(), "login_window");
        assert!(channel.verify().await.is_err());
        tokio::fs::write(workspace.path().join("login-step-3"), "")
            .await
            .unwrap();
        channel.verify().await.unwrap();
        assert!(channel.verify().await.is_err());
        assert_eq!(
            tokio::fs::metadata(workspace.path().join("login-verify"))
                .await
                .unwrap()
                .len(),
            0
        );
        assert!(workspace.cleanup().await);
        assert!(channel.phase().await.is_err());
    }

    #[tokio::test]
    async fn channel_never_forwards_unknown_or_oversized_worker_metadata() {
        let workspace = ConsoleBrowserWorkspace::create().await.unwrap();
        let channel = workspace.interaction();
        for content in ["private worker detail".to_owned(), "x".repeat(100)] {
            tokio::fs::write(workspace.path().join("login-step-3"), content)
                .await
                .unwrap();
            assert!(channel.phase().await.is_err());
            assert!(channel.verify().await.is_err());
        }
        assert!(workspace.cleanup().await);
    }

    #[tokio::test]
    async fn consent_racing_cleanup_cannot_recreate_a_workspace() {
        let workspace = ConsoleBrowserWorkspace::create().await.unwrap();
        let root = workspace.path().to_path_buf();
        let channel = workspace.interaction();
        tokio::fs::write(root.join("login-step-3"), "")
            .await
            .unwrap();
        let (_, removed) = tokio::join!(channel.verify(), workspace.cleanup());
        assert!(removed);
        assert!(!root.exists());
        assert!(channel.verify().await.is_err());
        assert!(!root.exists());
    }
}
