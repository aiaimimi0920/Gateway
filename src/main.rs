use neuro_gateway::config::{Config, GatewayRuntimeRole};

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
Usage: neuro-gateway [OPTIONS]

Environment-driven runtime:
  GATEWAY_RUNTIME_ROLE   splitter | worker | standalone
  GATEWAY_REDIS_URL      required Redis connection string
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if maybe_handle_meta_cli() {
        return Ok(());
    }

    let _ = rustls::crypto::ring::default_provider().install_default();
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

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
