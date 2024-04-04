mod handler;

use envoy_types::pb::envoy::service::ext_proc::v3::external_processor_server::ExternalProcessorServer;
use mt_ext_proc_grpc::ext_proc::ProcessingRequestHandlerServer;
use tokio::signal::unix::{signal, SignalKind};
use tonic::transport::Server;
use tracing::*;
use tracing_subscriber::{filter, prelude::*};

pub static PROJ_PRETTY_NAME: &str = env!("CARGO_PKG_NAME");
pub static PROJ_VERSION: &str = env!("CARGO_PKG_VERSION");
pub static BIN_PRETTY_NAME: &str = env!("CARGO_BIN_NAME");
pub static BIN_CODE_NAME: &str = env!("CARGO_CRATE_NAME");

async fn wait_for_signals() {
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = sigterm.recv() => tracing::info!("Received SIGTERM."),
        _ = sigint.recv() => tracing::info!("Received SIGINT."),
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::registry()
        .with(
            filter::Targets::new()
                .with_default(Level::INFO)
                .with_target(mt_ext_proc_grpc::LIB_CODE_NAME, Level::TRACE) // this package's library crate
                .with_target(BIN_CODE_NAME, Level::TRACE), // this binary crate
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let h = handler::MyExtProcHandler::new().await?;
    let server = ProcessingRequestHandlerServer::new(h);

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        // Set initial health value to high
        .set_serving::<ExternalProcessorServer<ProcessingRequestHandlerServer<handler::MyExtProcHandler>>>()
        .await;

    let addr = "0.0.0.0:50051".parse()?;
    info!(%addr, "listening");

    Server::builder()
        .trace_fn(|_| tracing::debug_span!("grpc_server"))
        .add_service(health_service)
        .add_service(ExternalProcessorServer::new(server))
        .serve_with_shutdown(addr, wait_for_signals())
        .await?;

    Ok(())
}
