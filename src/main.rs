#![feature(let_chains)]

use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tokio::sync::mpsc;
use envoy_types::{
    pb::envoy::service::ext_proc::v3::{
        external_processor_server::{ExternalProcessor, ExternalProcessorServer},
        ProcessingRequest, ProcessingResponse,
    },
};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tracing::*;
use tracing_subscriber::{filter, prelude::*};

pub static PKG: &str = env!("CARGO_PKG_NAME");
pub static NAME: &str = env!("CARGO_BIN_NAME"); // clap only has a macro for crate name
pub static VERSION: &str = clap::crate_version!();

#[derive(Default)]
struct MyExtProc;

fn map(req: ProcessingRequest) -> ProcessingResponse {
    println!("{:#?}", req);

    assert!(!req.async_mode);

    ProcessingResponse{
        dynamic_metadata: None,
        mode_override: None,
        override_message_timeout: None,
        response: None,
    }
}


#[tonic::async_trait]
impl ExternalProcessor for MyExtProc {

    type ProcessStream = ReceiverStream<Result<ProcessingResponse, Status>>;

    async fn process(
        &self,
        request: Request<Streaming<ProcessingRequest>>
    ) -> Result<Response<Self::ProcessStream>, Status>
    {
        let mut ins = request.into_inner();
        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            while let Some(result) = ins.next().await {
                match result {
                    Ok(v) => tx.send(Ok(map(v))).await.expect("working rx"),
                    Err(err) => {
                        // For better error handling see https://github.com/hyperium/tonic/blob/master/examples/src/streaming/server.rs
                        match tx.send(Err(err)).await {
                            Ok(_) => (),
                            Err(_err) => break,
                        }
                    }
                }
            }
        });

        let outs = ReceiverStream::new(rx);
        Ok(Response::new(outs))
    }
}

        // let client_headers = request.get_client_headers().ok_or(Status::invalid_argument("client headers not populated by envoy"))?;

        // let authz_decision = if
        //     let Some(h) = client_headers.get("x-let-me-in")
        //     && h == "pls"
        // {
        //     Status::ok("Welcome")
        // } else {
        //     Status::unauthenticated("Not authorized")
        // };

        // info!(?authz_decision, "Decided");

        // Ok(Response::new(CheckResponse::with_status(authz_decision)))

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::registry()
        .with(
            filter::Targets::new()
                .with_default(Level::INFO)
                .with_target(PKG, Level::INFO) // the library package
                .with_target(NAME, Level::INFO), // this binary package
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let addr = "0.0.0.0:50051".parse()?;
    let server = MyExtProc;

    info!(%addr, "AuthorizationServer listening");

    Server::builder().add_service(ExternalProcessorServer::new(server)).serve(addr).await?;

    Ok(())
}
