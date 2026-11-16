use github_copilot_sdk::{ClientMode, ClientOptions, Transport};
use std::path::PathBuf;

/// 路径来自受信任的运维配置，不接受推理请求覆盖。
pub struct RuntimeConfig {
    pub executable: PathBuf,
    pub directory: PathBuf,
}

impl RuntimeConfig {
    pub(crate) fn options(&self) -> Result<ClientOptions, std::io::Error> {
        if !self.executable.is_absolute() || !self.directory.is_absolute() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Copilot executable and isolated directory must be absolute",
            ));
        }
        if !self.executable.is_file() || !self.directory.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Copilot executable or isolated directory does not exist",
            ));
        }
        // 不自动发现 CLI，不创建用户目录，不读取环境账号；目录必须由上层分配并隔离。
        Ok(ClientOptions::default()
            .with_program(self.executable.clone())
            .with_transport(Transport::Stdio)
            .with_mode(ClientMode::Empty)
            .with_use_logged_in_user(false)
            .with_disable_client_telemetry(true)
            .with_cwd(&self.directory)
            .with_base_directory(&self.directory)
            .with_env_remove([
                "GH_TOKEN",
                "GITHUB_TOKEN",
                "COPILOT_GITHUB_TOKEN",
                "COPILOT_SDK_AUTH_TOKEN",
            ])
            .with_env([("COPILOT_TELEMETRY_DISABLED", "true")]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use github_copilot_sdk::{CliProgram, ClientMode, Transport};

    #[test]
    fn explicit_options_do_not_inherit_cli_defaults() {
        let directory = std::env::current_dir().unwrap();
        let executable = std::env::current_exe().unwrap();
        let options = RuntimeConfig {
            executable: executable.clone(),
            directory: directory.clone(),
        }
        .options()
        .unwrap();
        assert!(matches!(options.mode, ClientMode::Empty));
        assert!(matches!(options.transport, Transport::Stdio));
        assert!(matches!(options.program, CliProgram::Path(path) if path == executable));
        assert_eq!(options.use_logged_in_user, Some(false));
        assert_eq!(options.base_directory, Some(directory));
        assert!(options.github_token.is_none());
    }

    #[test]
    fn relative_paths_are_rejected_before_starting_a_process() {
        let config = RuntimeConfig {
            executable: "copilot.exe".into(),
            directory: ".".into(),
        };
        assert!(config.options().is_err());
    }
}
