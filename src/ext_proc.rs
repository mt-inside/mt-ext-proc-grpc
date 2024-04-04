// TODO PR to envoy-types crate
use std::{collections::hash_map::HashMap, error::Error, io::ErrorKind, sync::Arc};

use envoy_types::pb::{
    envoy::{
        config::core::v3::{HeaderMap, Metadata},
        extensions::filters::http::ext_proc::v3::ProcessingMode,
        service::ext_proc::v3::{
            external_processor_server::ExternalProcessor, processing_request::Request as ProcRequest, processing_response::Response as ProcResponse, BodyResponse, CommonResponse, HeaderMutation,
            HeadersResponse, ProcessingRequest, ProcessingResponse, TrailersResponse,
        },
    },
    google::protobuf::Struct,
};
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::{Request, Response, Status, Streaming};
use tracing::*;

#[tonic::async_trait]
pub trait ProcessingRequestHandler: Send + Sync + 'static {
    async fn request_headers(
        &self,
        headers: &HeaderMap,
        metadata_context: Option<Metadata>,
        attributes: HashMap<String, Struct>,
    ) -> (
        CommonResponse, // TODO: flatten
        Option<Struct>,
        Option<ProcessingMode>,
    );

    async fn response_headers(&self, headers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>, Option<ProcessingMode>);

    async fn request_body(&self, body: &[u8], metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>);

    async fn response_body(&self, body: &[u8], metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>);

    async fn request_trailers(&self, trailers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (Option<HeaderMutation>, Option<Struct>);

    async fn response_trailers(&self, trailers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (Option<HeaderMutation>, Option<Struct>);
}

#[derive(Debug)]
struct _Inner<T>(Arc<T>);
#[derive(Debug)]
pub struct ProcessingRequestHandlerServer<T: ProcessingRequestHandler + std::fmt::Debug> {
    handler: _Inner<T>,
}
impl<T: ProcessingRequestHandler + std::fmt::Debug> ProcessingRequestHandlerServer<T> {
    pub fn new(handler: T) -> Self {
        Self { handler: _Inner(Arc::new(handler)) }
    }

    async fn dispatch(&self, req: ProcessingRequest) -> ProcessingResponse {
        assert!(!req.async_mode);

        self.dispatch_inner(req).await.unwrap_or_default()
    }

    async fn dispatch_inner(&self, req: ProcessingRequest) -> Option<ProcessingResponse> {
        match req.request.as_ref()? {
            ProcRequest::RequestHeaders(hs) => {
                let (common_resp, dynamic_metadata, mode_override) = self.handler.0.request_headers(hs.headers.as_ref()?, req.metadata_context, req.attributes).await;

                Some(ProcessingResponse {
                    response: Some(ProcResponse::RequestHeaders(HeadersResponse { response: Some(common_resp) })),
                    dynamic_metadata,
                    mode_override,
                    override_message_timeout: None,
                })
            }
            ProcRequest::ResponseHeaders(hs) => {
                let (common_resp, dynamic_metadata, mode_override) = self.handler.0.response_headers(hs.headers.as_ref()?, req.metadata_context, req.attributes).await;

                Some(ProcessingResponse {
                    response: Some(ProcResponse::ResponseHeaders(HeadersResponse { response: Some(common_resp) })),
                    dynamic_metadata,
                    mode_override,
                    override_message_timeout: None,
                })
            }
            ProcRequest::RequestBody(b) => {
                let (common_resp, dynamic_metadata) = self.handler.0.request_body(&b.body, req.metadata_context, req.attributes).await;

                Some(ProcessingResponse {
                    response: Some(ProcResponse::RequestBody(BodyResponse { response: Some(common_resp) })),
                    dynamic_metadata,
                    mode_override: None, // Ignored in body & trailer responses
                    override_message_timeout: None,
                })
            }
            ProcRequest::ResponseBody(b) => {
                let (common_resp, dynamic_metadata) = self.handler.0.response_body(&b.body, req.metadata_context, req.attributes).await;

                Some(ProcessingResponse {
                    response: Some(ProcResponse::ResponseBody(BodyResponse { response: Some(common_resp) })),
                    dynamic_metadata,
                    mode_override: None, // Ignored in body & trailer responses
                    override_message_timeout: None,
                })
            }
            ProcRequest::RequestTrailers(ts) => {
                let (header_mutation, dynamic_metadata) = self.handler.0.request_trailers(ts.trailers.as_ref()?, req.metadata_context, req.attributes).await;

                Some(ProcessingResponse {
                    response: Some(ProcResponse::RequestTrailers(TrailersResponse { header_mutation })),
                    dynamic_metadata,
                    mode_override: None, // Ignored in body & trailer responses
                    override_message_timeout: None,
                })
            }
            ProcRequest::ResponseTrailers(ts) => {
                let (header_mutation, dynamic_metadata) = self.handler.0.response_trailers(ts.trailers.as_ref()?, req.metadata_context, req.attributes).await;

                Some(ProcessingResponse {
                    response: Some(ProcResponse::ResponseTrailers(TrailersResponse { header_mutation })),
                    dynamic_metadata,
                    mode_override: None, // Ignored in body & trailer responses
                    override_message_timeout: None,
                })
            }
        }
    }
}

impl<T: ProcessingRequestHandler + std::fmt::Debug> Clone for ProcessingRequestHandlerServer<T> {
    fn clone(&self) -> Self {
        let handler = self.handler.clone();
        Self { handler }
    }
}
impl<T: ProcessingRequestHandler + std::fmt::Debug> Clone for _Inner<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

#[tonic::async_trait]
impl<T: ProcessingRequestHandler + std::fmt::Debug> ExternalProcessor for ProcessingRequestHandlerServer<T> {
    type ProcessStream = ReceiverStream<Result<ProcessingResponse, Status>>;

    #[tracing::instrument(skip_all)]
    async fn process(&self, request: Request<Streaming<ProcessingRequest>>) -> Result<Response<Self::ProcessStream>, Status> {
        debug!("processing grpc stream item");

        let mut ins = request.into_inner();
        let (tx, rx) = mpsc::channel(128);

        let this = self.clone();
        tokio::spawn(async move {
            while let Some(result) = ins.next().await {
                match result {
                    Ok(v) => tx.send(Ok(this.dispatch(v).await)).await.expect("working rx"),
                    Err(err) => {
                        if let Some(io_err) = as_io_error(&err) {
                            if io_err.kind() == ErrorKind::BrokenPipe {
                                error!(?io_err, "client disconnected: broken pipe");
                                // TODO: reconnect
                                break;
                            }
                        }

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

fn as_io_error(err_status: &Status) -> Option<&std::io::Error> {
    let mut err: &(dyn Error + 'static) = err_status;

    loop {
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            return Some(io_err);
        }

        // h2::Error does not expose std::io::Error with `source()`
        // https://github.com/hyperium/h2/pull/462
        if let Some(h2_err) = err.downcast_ref::<h2::Error>() {
            if let Some(io_err) = h2_err.get_io() {
                return Some(io_err);
            }
        }

        err = match err.source() {
            Some(err) => err,
            None => return None,
        };
    }
}
