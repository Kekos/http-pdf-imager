use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::Path;
use tempfile::NamedTempFile;
use zip::write::SimpleFileOptions;

pub enum ZipError {
    Io(io::Error),
    IoWrite(io::Error),
    IoBuffer(io::Error),
    ZipLib(zip::result::ZipError),
}

pub fn write_to_zip<P: AsRef<Path>>(
    zip_path: P,
    files: std::slice::Iter<'_, NamedTempFile>,
) -> Result<(), ZipError> {
    let file = File::create(zip_path).map_err(ZipError::Io)?;
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for temp_image_file in files {
        let mut file_buffer = Vec::new();
        let file_name = temp_image_file
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        io::copy(&mut temp_image_file.take(u64::MAX), &mut file_buffer)
            .map_err(ZipError::IoBuffer)?;

        archive
            .start_file(file_name, options)
            .map_err(ZipError::ZipLib)?;
        archive.write_all(&file_buffer).map_err(ZipError::IoWrite)?;
    }

    archive.finish().map_err(ZipError::ZipLib)?;

    Ok(())
}
