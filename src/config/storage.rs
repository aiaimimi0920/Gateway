//! Backend selection is captured once; desktop UI location does not choose server storage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GatewayStorageMode {
    Local,
    #[default]
    Server,
}

impl GatewayStorageMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "server" => Ok(Self::Server),
            _ => Err("GATEWAY_STORAGE_MODE must be local or server".into()),
        }
    }

    pub fn resolve(
        explicit: Option<&str>,
        desktop_managed: bool,
        legacy: Option<&str>,
    ) -> Result<Self, String> {
        let explicit = explicit.map(Self::parse).transpose()?;
        if desktop_managed {
            if explicit == Some(Self::Server) {
                return Err("Desktop-managed backends require local storage".into());
            }
            return Ok(Self::Local);
        }
        Ok(explicit.unwrap_or(if legacy == Some("local") {
            Self::Local
        } else {
            Self::Server
        }))
    }

    pub fn from_env() -> Result<Self, String> {
        Self::resolve(
            std::env::var("GATEWAY_STORAGE_MODE").ok().as_deref(),
            std::env::var("GATEWAY_DESKTOP_MANAGED").as_deref() == Ok("1"),
            std::env::var("GATEWAY_CONSOLE_STORAGE").ok().as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::GatewayStorageMode as Mode;

    #[test]
    fn storage_selection_is_explicit_and_preserves_legacy_callers() {
        assert_eq!(Mode::resolve(None, false, None).unwrap(), Mode::Server);
        assert_eq!(
            Mode::resolve(None, false, Some("local")).unwrap(),
            Mode::Local
        );
        assert_eq!(
            Mode::resolve(Some("server"), false, Some("local")).unwrap(),
            Mode::Server
        );
        assert_eq!(
            Mode::resolve(Some(" local "), false, None).unwrap(),
            Mode::Local
        );
        assert_eq!(Mode::resolve(None, true, None).unwrap(), Mode::Local);
        assert!(Mode::resolve(Some("server"), true, None).is_err());
        assert!(Mode::resolve(Some("unknown"), false, None).is_err());
    }
}
