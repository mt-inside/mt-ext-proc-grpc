#![feature(let_chains)]

/* TODO
 * - handle kube graceful shutdown (sig 15?)
 * - copy better error handling from the example
 */

mod ext_proc;

use std::collections::hash_map::HashMap;

use envoy_types::pb::{
    envoy::{
        config::core::v3::{HeaderMap, HeaderValue, HeaderValueOption, Metadata},
        extensions::filters::http::ext_proc::v3::ProcessingMode,
        service::ext_proc::v3::{common_response::ResponseStatus, external_processor_server::ExternalProcessorServer, CommonResponse, HeaderMutation},
    },
    google::protobuf::Struct,
};
use ext_proc::{ProcessingRequestHandler, ProcessingRequestHandlerServer};
use tokio::signal::unix::{signal, SignalKind};
use tonic::transport::Server;
use tracing::*;
use tracing_subscriber::{filter, prelude::*};

pub static PKG: &str = env!("CARGO_PKG_NAME");
pub static NAME: &str = env!("CARGO_BIN_NAME"); // clap only has a macro for crate name
pub static VERSION: &str = clap::crate_version!();

#[derive(Default)]
struct MyExtProcHandler;

impl ProcessingRequestHandler for MyExtProcHandler {
    fn request_headers(&self, headers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>, Option<ProcessingMode>) {
        println!("== REQUEST HEADERS ==");
        println!("Dynamic metadata: {:?}", metadata_context);
        println!("Attributes: {:?}", attributes);
        println!("HEADERS");
        print_headers(headers);
        println!();

        (
            CommonResponse {
                status: ResponseStatus::Continue.into(),
                header_mutation: Some(HeaderMutation {
                    set_headers: vec![HeaderValueOption {
                        header: Some(HeaderValue {
                            key: NAME.to_owned(),
                            value: "request headers".to_owned(),
                            ..HeaderValue::default()
                        }),
                        ..HeaderValueOption::default()
                    }],
                    remove_headers: vec![],
                }),
                body_mutation: None,
                trailers: None,
                clear_route_cache: false,
            },
            None,
            None,
        )
    }

    fn response_headers(&self, headers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>, Option<ProcessingMode>) {
        println!("== RESPONSE HEADERS ==");
        println!("Dynamic metadata: {:?}", metadata_context);
        println!("Attributes: {:?}", attributes);
        println!("HEADERS");
        print_headers(headers);
        println!();

        (
            CommonResponse {
                status: ResponseStatus::Continue.into(),
                header_mutation: Some(HeaderMutation {
                    set_headers: vec![HeaderValueOption {
                        header: Some(HeaderValue {
                            key: NAME.to_owned(),
                            value: "response headers".to_owned(),
                            ..HeaderValue::default()
                        }),
                        ..HeaderValueOption::default()
                    }],
                    remove_headers: vec![],
                }),
                body_mutation: None,
                trailers: None,
                clear_route_cache: false,
            },
            None,
            None,
        )
    }

    fn request_body(&self, body: &[u8], metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>) {
        println!("== REQUEST BODY ==");
        println!("Dynamic metadata: {:?}", metadata_context);
        println!("Attributes: {:?}", attributes);
        println!("BODY");
        print_body(body);
        println!();

        (CommonResponse::default(), None)
    }

    fn response_body(&self, body: &[u8], metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>) {
        println!("== RESPONSE BODY ==");
        println!("Dynamic metadata: {:?}", metadata_context);
        println!("Attributes: {:?}", attributes);
        println!("BODY");
        print_body(body);
        println!();

        (CommonResponse::default(), None)
    }

    fn request_trailers(&self, trailers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (Option<HeaderMutation>, Option<Struct>) {
        println!("== REQUEST TRAILERS ==");
        println!("Dynamic metadata: {:?}", metadata_context);
        println!("Attributes: {:?}", attributes);
        println!("TRAILERS");
        print_headers(trailers);
        println!();

        (None, None)
    }

    fn response_trailers(&self, trailers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (Option<HeaderMutation>, Option<Struct>) {
        println!("== RESPONSE TRAILERS ==");
        println!("Dynamic metadata: {:?}", metadata_context);
        println!("Attributes: {:?}", attributes);
        println!("TRAILERS");
        print_headers(trailers);
        println!();

        (None, None)
    }
}

fn print_headers(hs: &HeaderMap) {
    for h in &hs.headers {
        print!("{}: ", h.key);
        if !h.value.is_empty() {
            print!("{}", h.value);
        } else if let Ok(s) = String::from_utf8(h.raw_value.clone()) {
            print!("{}", s);
        } else {
            print!("<not utf8>");
        }
        println!();
    }
}

fn print_body(body: &[u8]) {
    println!("Body of len: {}", body.len());
}

async fn wait_for_signals() {
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = sigterm.recv() => tracing::debug!("Received SIGTERM."),
        _ = sigint.recv() => tracing::debug!("Received SIGINT."),
    }
}

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
    let server = ProcessingRequestHandlerServer::new(MyExtProcHandler);

    info!(%addr, "listening");

    Server::builder()
        .add_service(ExternalProcessorServer::new(server))
        .serve_with_shutdown(addr, wait_for_signals())
        .await?;

    Ok(())
}
