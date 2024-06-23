use crate::pdf_converter::{ConvertParams, OutputImageType, PdfConvertError, PdfConvertResult};
use crate::zip::{write_to_zip, ZipError};
use crate::AppState;
use serde::Serialize;
use std::io::Write;
use tempfile::Builder;
use tide::http::headers::{HeaderValue, HeaderValues, ACCEPT};
use tide::log::{error, info};
use tide::StatusCode::{BadRequest, InternalServerError, NotAcceptable, UnprocessableEntity};
use tide::{Body, Request, Response, StatusCode};

const PDF_MAGIC: &[u8] = b"%PDF";

#[derive(serde::Serialize)]
struct JsonProblemResponse {
    #[serde(rename = "type")]
    _type: String,
    title: String,
    status: u16,
    detail: Option<String>,
    instance: Option<String>,
}

pub async fn handle(mut request: Request<AppState>) -> tide::Result {
    if request.content_type().is_none() {
        return Ok(create_mime_error_response());
    }

    let mime = request.content_type().unwrap();
    if mime.essence() != "application/pdf" {
        return Ok(create_mime_error_response());
    }

    let params: tide::Result<ConvertParams> = request.query();
    if params.is_err() {
        return Ok(create_query_params_error_response());
    }

    let mut params = params.unwrap();

    read_accept_header(&request, &mut params);

    let body = request.body_bytes().await?;
    if !body.starts_with(PDF_MAGIC) {
        return Ok(create_magic_error_response());
    }

    info!("{}", params);

    let mut pdf_temp_file = Builder::new().prefix("hpi").suffix(".pdf").tempfile()?;

    pdf_temp_file.write_all(&body)?;

    let pdf_temp_file_path = pdf_temp_file.path();
    info!(
        "Wrote request body to {}",
        pdf_temp_file_path.to_str().unwrap_or("unknown path")
    );

    let state = request.state();

    let convert_result = state.pdf_converter.convert(pdf_temp_file_path, params);

    state.increase_conversion_counter();

    match convert_result {
        Ok(res) => create_success_response(res).await,
        Err(e) => Ok(create_convert_error_response(e)),
    }
}

fn read_accept_header(request: &Request<AppState>, params: &mut ConvertParams) {
    let accept = request
        .header(ACCEPT)
        .unwrap_or(&HeaderValues::from(
            "image/png".parse::<HeaderValue>().unwrap(),
        ))
        .to_string();

    params.allow_zip = accept.contains("application/zip");

    if accept.contains("image/gif") {
        params.output_type = OutputImageType::Gif;
    } else if accept.contains("image/jpeg") {
        params.output_type = OutputImageType::Jpeg;
    } else if accept.contains("image/webp") {
        params.output_type = OutputImageType::Webp;
    } else {
        params.output_type = OutputImageType::Png;
    }
}

async fn create_success_response(convert_result: PdfConvertResult) -> tide::Result {
    Ok(match convert_result {
        PdfConvertResult::Multi(multi_pages) => {
            let zip_temp_file = Builder::new().prefix("hpi").suffix(".zip").tempfile()?;
            let zip_temp_path = zip_temp_file.path();
            let zip_result = write_to_zip(zip_temp_path, multi_pages.to_iter());

            if let Err(zip_err) = zip_result {
                return Ok(create_zip_error_response(zip_err));
            }

            let body = Body::from_file(&zip_temp_path).await?;

            body.into()
        }
        PdfConvertResult::Single(path) => {
            let body = Body::from_file(&path).await?;

            body.into()
        }
        PdfConvertResult::Empty => create_empty_pdf_error_response(),
    })
}

fn create_mime_error_response() -> Response {
    let problem = JsonProblemResponse {
        _type: String::from("about:blank"),
        title: String::from("Request error"),
        status: NotAcceptable.into(),
        detail: Some(String::from(
            "The Content-Type of request must be \"application/pdf\"",
        )),
        instance: None,
    };

    create_response_with_json(NotAcceptable, &problem)
}

fn create_query_params_error_response() -> Response {
    let problem = JsonProblemResponse {
        _type: String::from("about:blank"),
        title: String::from("Request error"),
        status: BadRequest.into(),
        detail: Some(String::from("Bad query params")),
        instance: None,
    };

    create_response_with_json(BadRequest, &problem)
}

fn create_magic_error_response() -> Response {
    let problem = JsonProblemResponse {
        _type: String::from("about:blank"),
        title: String::from("Request error"),
        status: UnprocessableEntity.into(),
        detail: Some(String::from(
            "The uploaded file does not seem to be a PDF file",
        )),
        instance: None,
    };

    create_response_with_json(UnprocessableEntity, &problem)
}

fn create_convert_error_response(e: PdfConvertError) -> Response {
    let detail = match e {
        PdfConvertError::LibraryLoad(ref pdfium_error) => {
            error!("PDFium library load error: {pdfium_error}");

            format!("Failed loading the PDFium library: {pdfium_error}")
        }
        PdfConvertError::DocumentLoad(ref pdfium_error) => {
            error!("PDFium document load error: {pdfium_error}");

            format!("Failed loading the document binary: {pdfium_error}")
        }
        PdfConvertError::PageRender(ref pdfium_error) => {
            error!("PDFium page render error: {pdfium_error}");

            format!("Failed rendering the PDF page: {pdfium_error}")
        }
        PdfConvertError::ImageWrite(ref image_error) => {
            error!("Image write error: {image_error}");

            format!("Failed writing the PDF page as image: {image_error}")
        }
        PdfConvertError::ImageRead(ref image_error) => {
            error!("Image read error: {image_error}");

            format!("Failed read image: {image_error}")
        }
        PdfConvertError::TempFile(ref io_error) => {
            error!("Error when creating the temporary image file: {io_error}");

            String::from("Unknown file write error")
        }
    };

    let problem = JsonProblemResponse {
        _type: String::from("about:blank"),
        title: String::from("PDF convert error"),
        status: 500,
        detail: Some(detail),
        instance: None,
    };

    create_response_with_json(InternalServerError, &problem)
}

fn create_zip_error_response(e: ZipError) -> Response {
    let detail = match e {
        ZipError::Io(io_error) => {
            error!("IO error when creating ZIP archive: {io_error}");
            format!("ZIP IO error: {io_error}")
        }
        ZipError::IoWrite(io_error) => {
            error!("IO error when writing image buffer to ZIP: {io_error}");
            format!("ZIP write error: {io_error}")
        }
        ZipError::IoBuffer(io_error) => {
            error!("IO error when reading image file to buffer: {io_error}");
            format!("ZIP read to buffer error: {io_error}")
        }
        ZipError::ZipLib(zip_err) => {
            error!("ZIP library error: {zip_err}");
            format!("ZIP write error: {zip_err}")
        }
    };

    let problem = JsonProblemResponse {
        _type: String::from("about:blank"),
        title: String::from("ZIP write error"),
        status: 500,
        detail: Some(detail),
        instance: None,
    };

    create_response_with_json(InternalServerError, &problem)
}

fn create_empty_pdf_error_response() -> Response {
    let problem = JsonProblemResponse {
        _type: String::from("about:blank"),
        title: String::from("Request error"),
        status: BadRequest.into(),
        detail: Some(String::from(
            "No pages could be extracted from the PDF. Is it empty?",
        )),
        instance: None,
    };

    create_response_with_json(BadRequest, &problem)
}

fn create_response_with_json(status: StatusCode, json: &impl Serialize) -> Response {
    let mut response = Response::new(status);

    if let Ok(body) = Body::from_json(&json) {
        response.set_body(body);
    }

    response
}
