use crate::pdf_converter::PdfConverter;
use serde_json::json;
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tide::http::headers::AUTHORIZATION;
use tide::log::LogMiddleware;
use tide::{Body, Next, Request, Response, StatusCode};

mod http_post_controller;
mod pdf_converter;
mod zip;

#[derive(Clone)]
struct AppState {
    auth_token: String,
    pdf_converter: PdfConverter,
    count_conversions: Arc<AtomicUsize>,
}

impl AppState {
    fn new(auth_token: String, pdfium_lib: String) -> Self {
        Self {
            auth_token,
            pdf_converter: PdfConverter::new(pdfium_lib),
            count_conversions: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn increase_conversion_counter(&self) {
        self.count_conversions.fetch_add(1, Ordering::Acquire);
    }
}

fn auth_middleware(
    request: Request<AppState>,
    next: Next<'_, AppState>,
) -> Pin<Box<dyn Future<Output = tide::Result> + Send + '_>> {
    Box::pin(async {
        let auth_header = request.header(AUTHORIZATION);

        if auth_header.is_none() {
            return Ok(Response::new(StatusCode::Unauthorized));
        }

        let auth_header = auth_header.unwrap().as_str();
        let required_auth_value = format!("token {}", request.state().auth_token);

        if auth_header != required_auth_value {
            return Ok(Response::new(StatusCode::Unauthorized));
        }

        Ok(next.run(request).await)
    })
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    femme::start();

    let server_port = env::var("HPI_PORT").unwrap_or_else(|_e| "8507".to_string());
    let auth_token = env::var("HPI_AUTH_TOKEN").unwrap_or_else(|_e| "".to_string());
    let pdfium_lib = env::var("HPI_PDFIUM_LIB").unwrap_or_else(|_e| "".to_string());

    let mut app = tide::with_state(AppState::new(auth_token.clone(), pdfium_lib));

    app.with(LogMiddleware::new());

    if !auth_token.is_empty() {
        app.with(auth_middleware);
    }

    app.at("/").get(get);
    app.at("/").post(http_post_controller::handle);

    app.listen(vec![format!("0.0.0.0:{server_port}")]).await?;

    Ok(())
}

async fn get(request: Request<AppState>) -> tide::Result {
    let count_conversions = request.state().count_conversions.load(Ordering::Acquire);

    let mut response = Response::new(StatusCode::Ok);
    response.set_body(Body::from_json(&json!({
        "count_conversions": count_conversions,
    }))?);

    Ok(response)
}
