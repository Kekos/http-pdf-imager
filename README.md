# http-pdf-imager

An HTTP microservice for converting PDF files to images.
Uses the [PDFium](https://pdfium.googlesource.com/pdfium/) library with the
[pdfium-render crate](https://crates.io/crates/pdfium-render).
    
## Configuration

The app reads the following environment variables:

* `HPI_PORT` - The port which the application listens on, default `8507`.
* `HPI_AUTH_TOKEN` - Authorization token, default is empty, no authorization.
* `HPI_PDFIUM_LIB` - Path to PDFium library, defaults to empty.

## Usage



## License

MIT
