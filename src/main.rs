use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use constellation::http::{AppState, build_router};
use constellation::index::{Index, warm_up};

const ABOUT: &str = "A local web UI for browsing and resuming every Claude Code chat.";

#[derive(Debug, Parser)]
#[command(name = "cchats", version, about = ABOUT, long_about = None)]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 6767)]
    port: u16,

    #[arg(long)]
    no_open: bool,

    #[arg(long)]
    root: Option<PathBuf>,

    #[arg(long, default_value = "warn,constellation=info")]
    log: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&args.log).unwrap_or_else(|_| EnvFilter::new("warn")))
        .with_target(false)
        .compact()
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;

    rt.block_on(serve(args))
}

async fn serve(args: Args) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", args.host, args.port))?;

    if !is_loopback(&addr.ip()) {
        warn!(
            host = %args.host,
            "binding to a non-loopback address: the server is UNAUTHENTICATED \
             and exposes all sessions and the PTY spawn endpoint to any host \
             that can reach it",
        );
        eprintln!(
            "warning: --host {} is not loopback. constellation has no \
             authentication; any host that can reach this port can read all \
             chat history and spawn claude in your projects. use \
             --host 127.0.0.1 (the default) unless you intend this.",
            args.host
        );
    }

    let index = match args.root {
        Some(p) => Index::new(p),
        None => Index::default_location(),
    };

    let state = AppState::new(index.clone());
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    let url = format!("http://{}", addr);
    eprintln!("constellation → {url}");

    let warmup_idx = index.clone();
    tokio::spawn(async move {
        warm_up(&warmup_idx).await;
        info!("initial index ready");
    });

    if !args.no_open {
        let u = url.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let _ = open::that(u);
        });
    }

    axum::serve(listener, app).await.context("serving")?;
    Ok(())
}

fn is_loopback(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}
