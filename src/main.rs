use neuro_gateway::config::{Config, GatewayRuntimeRole};
mod local_installation;

const DEFAULT_TOKIO_WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;
const MIN_TOKIO_WORKER_STACK_BYTES: usize = 2 * 1024 * 1024;

fn maybe_handle_meta_cli() -> bool {
    let mut args = std::env::args();
    let _program = args.next();
    let first = match args.next() {
        Some(value) => value,
        None => return false,
    };

    if args.next().is_some() {
        return false;
    }

    match first.as_str() {
        "-h" | "--help" => {
            println!(
                "\
Usage: gateway [OPTIONS]

Environment-driven runtime:
  GATEWAY_RUNTIME_ROLE   splitter | worker | standalone
  GATEWAY_REDIS_URL      Redis connection string for server deployments
  GATEWAY_DATA_DIR       explicit local data root (default: exe/.ng if present, else ~/.ng)
  PORT                   bind port for worker / standalone runtime

Common validation entrypoints:
  powershell -File tools/verify-gateway-release-candidate.ps1
  powershell -File tools/verify-gateway-line.ps1 -All

Meta options:
  -h, --help             Print this help
  -V, --version          Print version"
            );
            true
        }
        "-V" | "--version" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            true
        }
        _ => false,
    }
}

fn parse_tokio_worker_stack_bytes(raw: Option<&str>) -> Result<usize, String> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_TOKIO_WORKER_STACK_BYTES);
    };

    let parsed = raw
        .parse::<usize>()
        .map_err(|error| format!("invalid integer '{raw}': {error}"))?;

    if parsed < MIN_TOKIO_WORKER_STACK_BYTES {
        return Err(format!(
            "value {parsed} is below the minimum supported {MIN_TOKIO_WORKER_STACK_BYTES} bytes"
        ));
    }

    Ok(parsed)
}

async fn async_main(worker_stack_bytes: usize) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!(
        worker_stack_bytes,
        "Tokio worker thread stack size configured"
    );

    let config = Config::from_env().map_err(|error| anyhow::anyhow!("Config error: {}", error))?;

    match config.runtime_role {
        GatewayRuntimeRole::Splitter => {
            neuro_gateway::splitter::SplitterManager::new(config)?
                .run()
                .await
        }
        GatewayRuntimeRole::Worker | GatewayRuntimeRole::Standalone => {
            neuro_gateway::runtime::run_gateway_runtime(config).await
        }
    }
}

fn main() -> anyhow::Result<()> {
    if maybe_handle_meta_cli() {
        return Ok(());
    }

    let _ = rustls::crypto::ring::default_provider().install_default();
    let local_data_lease = local_installation::configure()?;
    // Desktop-managed instances receive a complete, isolated environment.
    // Do not discover another deployment's .env in the package or its parents.
    if local_data_lease.is_none() && std::env::var("GATEWAY_DESKTOP_MANAGED").as_deref() != Ok("1")
    {
        dotenvy::dotenv().ok();
    }

    let worker_stack_bytes = match parse_tokio_worker_stack_bytes(
        std::env::var("GATEWAY_TOKIO_WORKER_STACK_BYTES")
            .ok()
            .as_deref(),
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "Invalid GATEWAY_TOKIO_WORKER_STACK_BYTES override ({error}); falling back to default {} bytes.",
                DEFAULT_TOKIO_WORKER_STACK_BYTES
            );
            DEFAULT_TOKIO_WORKER_STACK_BYTES
        }
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(worker_stack_bytes)
        .build()?;
    runtime.block_on(async_main(worker_stack_bytes))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_tokio_worker_stack_bytes, DEFAULT_TOKIO_WORKER_STACK_BYTES,
        MIN_TOKIO_WORKER_STACK_BYTES,
    };

    #[test]
    fn tokio_worker_stack_bytes_defaults_when_unset() {
        assert_eq!(
            parse_tokio_worker_stack_bytes(None).expect("default stack bytes"),
            DEFAULT_TOKIO_WORKER_STACK_BYTES
        );
    }

    #[test]
    fn tokio_worker_stack_bytes_accepts_valid_override() {
        assert_eq!(
            parse_tokio_worker_stack_bytes(Some("8388608")).expect("valid override"),
            8 * 1024 * 1024
        );
    }

    #[test]
    fn tokio_worker_stack_bytes_rejects_non_numeric_override() {
        let error =
            parse_tokio_worker_stack_bytes(Some("not-a-number")).expect_err("invalid override");
        assert!(error.contains("invalid integer"));
    }

    #[test]
    fn tokio_worker_stack_bytes_rejects_too_small_override() {
        let below_minimum = (MIN_TOKIO_WORKER_STACK_BYTES - 1).to_string();
        let error = parse_tokio_worker_stack_bytes(Some(&below_minimum))
            .expect_err("below-minimum override");
        assert!(error.contains("below the minimum supported"));
    }
}
