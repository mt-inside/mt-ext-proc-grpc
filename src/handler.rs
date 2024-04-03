use std::collections::hash_map::HashMap;

use envoy_types::pb::{
    envoy::{
        config::core::v3::{HeaderMap, HeaderValue, HeaderValueOption, Metadata},
        extensions::filters::http::ext_proc::v3::ProcessingMode,
        service::ext_proc::v3::{common_response::ResponseStatus, CommonResponse, HeaderMutation},
    },
    google::protobuf::Struct,
};
use mt_ext_proc_grpc::ext_proc::ProcessingRequestHandler;
use tracing::*;

#[derive(Default, Debug)]
pub struct MyExtProcHandler;

impl ProcessingRequestHandler for MyExtProcHandler {
    #[tracing::instrument(skip_all)]
    fn request_headers(&self, headers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>, Option<ProcessingMode>) {
        trace!("handling");
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
                            key: crate::BIN_PRETTY_NAME.to_owned(),
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

    #[tracing::instrument(skip_all)]
    fn response_headers(&self, headers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>, Option<ProcessingMode>) {
        trace!("handling");
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
                            key: crate::BIN_PRETTY_NAME.to_owned(),
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

    #[tracing::instrument(skip_all)]
    fn request_body(&self, body: &[u8], metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>) {
        trace!("handling");
        println!("== REQUEST BODY ==");
        println!("Dynamic metadata: {:?}", metadata_context);
        println!("Attributes: {:?}", attributes);
        println!("BODY");
        print_body(body);
        println!();

        (CommonResponse::default(), None)
    }

    #[tracing::instrument(skip_all)]
    fn response_body(&self, body: &[u8], metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (CommonResponse, Option<Struct>) {
        trace!("handling");
        println!("== RESPONSE BODY ==");
        println!("Dynamic metadata: {:?}", metadata_context);
        println!("Attributes: {:?}", attributes);
        println!("BODY");
        print_body(body);
        println!();

        (CommonResponse::default(), None)
    }

    #[tracing::instrument(skip_all)]
    fn request_trailers(&self, trailers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (Option<HeaderMutation>, Option<Struct>) {
        trace!("handling");
        println!("== REQUEST TRAILERS ==");
        println!("Dynamic metadata: {:?}", metadata_context);
        println!("Attributes: {:?}", attributes);
        println!("TRAILERS");
        print_headers(trailers);
        println!();

        (None, None)
    }

    #[tracing::instrument(skip_all)]
    fn response_trailers(&self, trailers: &HeaderMap, metadata_context: Option<Metadata>, attributes: HashMap<String, Struct>) -> (Option<HeaderMutation>, Option<Struct>) {
        trace!("handling");
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
