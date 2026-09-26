use crate::credential_pool_automation::{
    CredentialAutomationDriver, CredentialAutomationDriverTransport,
    CredentialPoolAutomationConfig, DriverCredentialContext, DriverProviderContext, DriverRequest,
};
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::Instant;
use uuid::Uuid;

pub(super) struct Fixture {
    directory: PathBuf,
    script: String,
}

pub(super) struct Observed {
    pub started: bool,
    pub exited_promptly: bool,
    pub request: Option<serde_json::Value>,
}

impl Fixture {
    pub fn new(mode: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("gateway-automation-script-{}", Uuid::new_v4()));
        fs::create_dir(&directory).expect("create unique automation script fixture directory");
        let (script, source) = if cfg!(windows) && mode == "closed-input" {
            (format!("{mode}.ps1"), include_str!("closed_input.ps1"))
        } else {
            (format!("{mode}.mjs"), include_str!("fixture.mjs"))
        };
        fs::write(directory.join(&script), source).expect("write automation script fixture");
        Self { directory, script }
    }

    pub fn config(&self) -> CredentialPoolAutomationConfig {
        CredentialPoolAutomationConfig {
            script_root: Some(self.directory.clone()),
            ..CredentialPoolAutomationConfig::default()
        }
    }

    pub fn driver(&self, timeout_secs: u64) -> CredentialAutomationDriver {
        CredentialAutomationDriver {
            id: "script-driver".to_string(),
            provider_ids: vec!["provider-a".to_string()],
            timeout_secs: Some(timeout_secs),
            transport: CredentialAutomationDriverTransport::Script {
                script: self.script.clone(),
            },
        }
    }

    fn pid(&self) -> Option<u32> {
        fs::read_to_string(self.directory.join(format!("{}.pid", self.script)))
            .ok()
            .and_then(|text| text.trim().parse().ok())
    }

    pub async fn wait_for_start(&self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.pid().is_none() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        self.pid().is_some()
    }

    pub async fn finish(self) -> Observed {
        let pid = self.pid();
        let started_at = Instant::now();
        let mut exited_promptly = false;
        // Even a failed baseline gets cleanup evidence before its assertions run.
        if let Some(pid) = pid {
            while started_at.elapsed() < Duration::from_secs(14) {
                if process_has_exited(pid).await {
                    exited_promptly = started_at.elapsed() < Duration::from_secs(2);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        let request = fs::read(self.directory.join(format!("{}.request.json", self.script)))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        Observed {
            started: pid.is_some(),
            exited_promptly,
            request,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

async fn process_has_exited(pid: u32) -> bool {
    let mut command = Command::new("node");
    command
        .args([
            "-e",
            "try { process.kill(Number(process.argv[1]), 0); process.exit(1); } catch (error) { process.exit(error.code === 'ESRCH' ? 0 : 2); }",
            &pid.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    matches!(tokio::time::timeout(Duration::from_secs(2), command.status()).await, Ok(Ok(status)) if status.success())
}

pub(super) fn request(large: bool) -> DriverRequest {
    DriverRequest {
        run_id: "script-contract".to_string(),
        action: "reconcile",
        provider: DriverProviderContext {
            id: "provider-a".to_string(),
            label: if large {
                "a".repeat(1024 * 1024)
            } else {
                "Provider A".to_string()
            },
            target_size: 2,
            credential_count: 1,
            active_credential_count: 1,
            requested_count: 1,
            auto_refill_enabled: true,
            auto_prune_enabled: false,
            identity_categories: vec![serde_json::json!({"id": "default"})],
            credentials: vec![DriverCredentialContext {
                id: "existing-a".to_string(),
                account_name: Some("Account A".to_string()),
                enabled: true,
                identity_category_id: Some("default".to_string()),
            }],
        },
        refill: None,
    }
}
