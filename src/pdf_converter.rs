use image::{imageops, ImageBuffer, ImageFormat, RgbImage};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium, PdfiumError, Pixels};
use std::fmt::{Display, Formatter};
use std::io;
use std::path::Path;
use tempfile::{Builder, NamedTempFile};

#[derive(serde::Deserialize)]
pub enum OutputImageType {
    Png,
    Gif,
    Jpeg,
    Webp,
}

impl Display for OutputImageType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            OutputImageType::Png => ".png",
            OutputImageType::Gif => ".gif",
            OutputImageType::Jpeg => ".jpg",
            OutputImageType::Webp => ".webp",
        }
        .to_string();
        write!(f, "{}", str)
    }
}

impl From<&OutputImageType> for ImageFormat {
    fn from(value: &OutputImageType) -> Self {
        match value {
            OutputImageType::Png => ImageFormat::Png,
            OutputImageType::Gif => ImageFormat::Gif,
            OutputImageType::Jpeg => ImageFormat::Jpeg,
            OutputImageType::Webp => ImageFormat::WebP,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(default)]
pub struct ConvertParams {
    #[serde(skip_deserializing)]
    pub output_type: OutputImageType,
    #[serde(skip_deserializing)]
    pub allow_zip: bool,
    pub dpi: u32,
    pub preserve_alpha: bool,
    pub background_color: String,
}

impl Default for ConvertParams {
    fn default() -> Self {
        Self {
            output_type: OutputImageType::Png,
            allow_zip: false,
            dpi: 72,
            preserve_alpha: false,
            background_color: "white".to_string(),
        }
    }
}

impl Display for ConvertParams {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        fmt.write_fmt(format_args!(
            "Type: {}, {}, DPI {}, {}, background: {}",
            self.output_type,
            match self.allow_zip {
                true => "Multi page to ZIP",
                false => "Multi page to image",
            },
            self.dpi,
            match self.preserve_alpha {
                true => "Preserve alpha",
                false => "Remove alpha",
            },
            self.background_color
        ))
    }
}

pub enum PdfConvertError {
    LibraryLoad(PdfiumError),
    DocumentLoad(PdfiumError),
    PageRender(PdfiumError),
    ImageWrite(image::ImageError),
    ImageRead(image::ImageError),
    TempFile(io::Error),
}

pub struct MultiPagesResult {
    temp_files: Vec<NamedTempFile>,
}

impl MultiPagesResult {
    fn new() -> Self {
        Self {
            temp_files: Vec::new(),
        }
    }

    fn push(&mut self, path: NamedTempFile) {
        self.temp_files.push(path);
    }

    fn is_empty(&self) -> bool {
        self.temp_files.is_empty()
    }

    fn is_single(&self) -> bool {
        self.temp_files.len() == 1
    }

    pub fn to_iter(&self) -> std::slice::Iter<'_, NamedTempFile> {
        self.temp_files.iter()
    }
}

pub enum PdfConvertResult {
    Empty,
    Single(NamedTempFile),
    Multi(MultiPagesResult),
}

#[derive(Clone)]
pub struct PdfConverter {
    pdfium_lib: String,
}

impl PdfConverter {
    pub fn new(pdfium_lib: String) -> Self {
        Self { pdfium_lib }
    }

    pub fn convert(
        &self,
        pdf_file_path: &Path,
        params: ConvertParams,
    ) -> Result<PdfConvertResult, PdfConvertError> {
        let pdfium = Pdfium::new(
            Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
                &self.pdfium_lib,
            ))
            .or_else(|_| Pdfium::bind_to_system_library())
            .map_err(PdfConvertError::LibraryLoad)?,
        );

        let document = pdfium
            .load_pdf_from_file(pdf_file_path, None)
            .map_err(PdfConvertError::DocumentLoad)?;

        let mut result = MultiPagesResult::new();
        let dpi = params.dpi as f32;
        let image_format = ImageFormat::from(&params.output_type);

        for (index, page) in document.pages().iter().enumerate() {
            let image_temp_file = Builder::new()
                .prefix(&format!("{index}-hpi"))
                .suffix(&params.output_type.to_string())
                .tempfile()
                .map_err(PdfConvertError::TempFile)?;
            let image_temp_path = image_temp_file.path();

            let width_inches = page.width().to_inches();
            let width_px = width_inches * dpi;
            let render_config = PdfRenderConfig::new().set_target_width(width_px.round() as Pixels);

            page.render_with_config(&render_config)
                .map_err(PdfConvertError::PageRender)?
                .as_image()
                .into_rgb8()
                .save_with_format(image_temp_path, image_format)
                .map_err(PdfConvertError::ImageWrite)?;

            result.push(image_temp_file);
        }

        if result.is_empty() {
            return Ok(PdfConvertResult::Empty);
        }

        if !params.allow_zip {
            if result.is_single() {
                let first = result.temp_files.remove(0);

                return Ok(PdfConvertResult::Single(first));
            }

            return Ok(PdfConvertResult::Single(combine_images(
                result,
                image_format,
            )?));
        }

        Ok(PdfConvertResult::Multi(result))
    }
}

fn combine_images(
    result: MultiPagesResult,
    format: ImageFormat,
) -> Result<NamedTempFile, PdfConvertError> {
    let image_temp_file = Builder::new()
        .prefix("hpi")
        .tempfile()
        .map_err(PdfConvertError::TempFile)?;

    let image_temp_path = image_temp_file.path();

    let image_files = result.temp_files.iter();
    let mut images: Vec<RgbImage> = Vec::new();
    let mut width = 0i64;
    let mut height = 0i64;

    for image_file in image_files {
        let image = image::open(image_file.path()).map_err(PdfConvertError::ImageRead)?.into_rgb8();
        let image_width = image.width() as i64;

        height += image.height() as i64;
        images.push(image);

        if image_width > width {
            width = image_width;
        }
    }

    let mut image_buffer = ImageBuffer::new(width as u32, height as u32);
    let mut offset_y = 0;

    for image in images {
        imageops::overlay(&mut image_buffer, &image, 0, offset_y);

        offset_y += image.height() as i64;
    }

    image_buffer
        .save_with_format(image_temp_path, format)
        .map_err(PdfConvertError::ImageWrite)?;

    Ok(image_temp_file)
}
