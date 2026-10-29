//! Parent-owned scratch space survives cancellation until the browser supervisor stops.
use std::{io, path::PathBuf, time::Duration};
use tokio::process::Command;

pub(crate) struct ConsoleBrowserWorkspace {
    root: Option<PathBuf>,
}

impl ConsoleBrowserWorkspace {
    pub(crate) async fn create() -> io::Result<Self> {
        // Allocation and ownership transfer must finish together, even if the caller is cancelled.
        tokio::task::spawn_blocking(|| {
            let parent = std::fs::canonicalize(std::env::temp_dir())?;
            let root = parent.join(format!("gateway-console-browser-{}", uuid::Uuid::new_v4()));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&root)?;
            Ok(Self { root: Some(root) })
        })
        .await
        .map_err(io::Error::other)?
    }

    pub(super) fn configure(&self, command: &mut Command) {
        if let Some(root) = &self.root {
            command
                .env("TEMP", root)
                .env("TMP", root)
                .env("TMPDIR", root)
                .env("GATEWAY_CHATGPT_LOGIN_CHANNEL", root);
        }
    }

    pub(crate) fn interaction(&self) -> super::ConsoleBrowserInteraction {
        super::ConsoleBrowserInteraction::new(
            self.root.as_ref().expect("owned scratch root").clone(),
        )
    }

    pub(super) async fn cleanup(mut self) -> bool {
        let removed = match &self.root {
            Some(root) => remove_owned(root).await,
            None => true,
        };
        if removed {
            self.root.take();
        }
        removed
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &std::path::Path {
        self.root.as_deref().expect("owned scratch root")
    }
}

async fn remove_owned(root: &std::path::Path) -> bool {
    // Only our freshly-created absolute directory reaches here. Never follow a replaced root.
    for attempt in 0..20 {
        match tokio::fs::canonicalize(root).await {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return true,
            Ok(resolved) if resolved != root => return false,
            Ok(_) => match tokio::fs::remove_dir_all(root).await {
                Ok(()) => return true,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return true,
                Err(_) => {}
            },
            Err(_) => {}
        }
        if attempt < 19 {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
    false
}

impl Drop for ConsoleBrowserWorkspace {
    fn drop(&mut self) {
        let Some(root) = self.root.take() else { return };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if !remove_owned(&root).await {
                    tracing::warn!(
                        code = "console_browser_scratch_cleanup_failed",
                        "Browser scratch cleanup did not complete"
                    );
                }
            });
        }
    }
}

#[cfg(test)]
#[path = "console_browser_workspace_tests.rs"]
mod tests;
