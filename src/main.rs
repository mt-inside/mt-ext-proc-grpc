#![feature(let_chains)]

use envoy_types::pb::envoy::{
    config::core::v3::HeaderMap,
    service::ext_proc::v3::{
        common_response::ResponseStatus,
        external_processor_server::{ExternalProcessor, ExternalProcessorServer},
        processing_request::Request as ProcRequest,
        processing_response::Response as ProcResponse,
        BodyResponse, CommonResponse, HeadersResponse, HttpBody, ProcessingRequest, ProcessingResponse, TrailersResponse,
    },
};
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tracing::*;
use tracing_subscriber::{filter, prelude::*};

pub static PKG: &str = env!("CARGO_PKG_NAME");
pub static NAME: &str = env!("CARGO_BIN_NAME"); // clap only has a macro for crate name
pub static VERSION: &str = clap::crate_version!();

#[derive(Default)]
struct MyExtProc;

fn map(req: ProcessingRequest) -> ProcessingResponse {
    assert!(!req.async_mode);

    let resp = map_inner(req).unwrap_or_default();
    resp
}

fn map_inner(req: ProcessingRequest) -> Option<ProcessingResponse> {
    // TODO: make a trait for this that people impl: ProcessingHandler{fn request_headers(metadata_context, attrs, req:HttpHeaders) -> CommonResponse, etc}
    // - and a fn that takes one and dispatches to it as below
    // - PR to envoy-types crate
    match req.request.as_ref()? {
        ProcRequest::RequestHeaders(hs) => {
            println!("== REQUEST HEADERS ==");
            print_common(&req);
            println!("Body follows: {}", !hs.end_of_stream);
            println!("HEADERS");
            if let Some(hs) = &hs.headers {
                print_headers(hs);
            }
            println!();
            Some(ProcessingResponse {
                response: Some(ProcResponse::RequestHeaders(HeadersResponse {
                    response: Some(CommonResponse {
                        status: ResponseStatus::Continue.into(),
                        header_mutation: None,
                        body_mutation: None,
                        trailers: None,
                        clear_route_cache: false,
                    }),
                })),
                ..ProcessingResponse::default()
            })
        }
        ProcRequest::ResponseHeaders(hs) => {
            println!("== RESPONSE HEADERS ==");
            print_common(&req);
            println!("Body follows: {}", !hs.end_of_stream);
            println!("HEADERS");
            if let Some(hs) = &hs.headers {
                print_headers(hs);
            }
            println!();
            Some(ProcessingResponse {
                response: Some(ProcResponse::ResponseHeaders(HeadersResponse {
                    response: Some(CommonResponse {
                        status: ResponseStatus::Continue.into(),
                        header_mutation: None,
                        body_mutation: None,
                        trailers: None,
                        clear_route_cache: false,
                    }),
                })),
                ..ProcessingResponse::default()
            })
        }
        ProcRequest::RequestBody(b) => {
            println!("== REQUEST BODY ==");
            print_common(&req);
            println!("Trailers follow: {}", !b.end_of_stream);
            println!("BODY");
            print_body(b);
            println!();
            Some(ProcessingResponse {
                response: Some(ProcResponse::RequestBody(BodyResponse {
                    response: Some(CommonResponse {
                        status: ResponseStatus::Continue.into(),
                        header_mutation: None,
                        body_mutation: None,
                        trailers: None,
                        clear_route_cache: false,
                    }),
                })),
                ..ProcessingResponse::default()
            })
        }
        ProcRequest::ResponseBody(b) => {
            println!("== RESPONSE BODY ==");
            print_common(&req);
            println!("Trailers follow: {}", !b.end_of_stream);
            println!("BODY");
            print_body(b);
            println!();
            Some(ProcessingResponse {
                response: Some(ProcResponse::ResponseBody(BodyResponse {
                    response: Some(CommonResponse {
                        status: ResponseStatus::Continue.into(),
                        header_mutation: None,
                        body_mutation: None,
                        trailers: None,
                        clear_route_cache: false,
                    }),
                })),
                ..ProcessingResponse::default()
            })
        }
        ProcRequest::RequestTrailers(ts) => {
            println!("== REQUEST TRAILERS ==");
            print_common(&req);
            println!("TRAILERS");
            if let Some(ts) = &ts.trailers {
                print_headers(ts);
            }
            println!();
            Some(ProcessingResponse {
                response: Some(ProcResponse::RequestTrailers(TrailersResponse { header_mutation: None })),
                ..ProcessingResponse::default()
            })
        }
        ProcRequest::ResponseTrailers(ts) => {
            println!("== RESPONSE TRAILERS ==");
            print_common(&req);
            println!("TRAILERS");
            if let Some(ts) = &ts.trailers {
                print_headers(ts);
            }
            println!();
            Some(ProcessingResponse {
                response: Some(ProcResponse::ResponseTrailers(TrailersResponse { header_mutation: None })),
                ..ProcessingResponse::default()
            })
        }
    }
}

fn print_common(req: &ProcessingRequest) {
    println!("Dynamic metadata: {:?}", req.metadata_context);
    println!("Attributes: {:?}", req.attributes);
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

fn print_body(body: &HttpBody) {
    println!("Body of len: {}", body.body.len());
}

#[tonic::async_trait]
impl ExternalProcessor for MyExtProc {
    type ProcessStream = ReceiverStream<Result<ProcessingResponse, Status>>;

    async fn process(&self, request: Request<Streaming<ProcessingRequest>>) -> Result<Response<Self::ProcessStream>, Status> {
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

    info!(%addr, "listening");

    Server::builder().add_service(ExternalProcessorServer::new(server)).serve(addr).await?;

    Ok(())
}
