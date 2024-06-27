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

The service can be started in Docker:

```bash
docker run --rm -p 8507:8507 keke91/http-pdf-imager
```

...or in Docker Compose:

```yaml
services:
  pdf-imager:
    image: keke91/http-pdf-imager
    ports:
      - "8507:8507"
    environment:
      HPI_AUTH_TOKEN: "my-secret-token"
```

## API

The PDF file to convert must be sent to the `/` endpoint through POST with an
`Content-Type: application/pdf` HTTP header.

To hint what type of image to convert to, set the `Accept` header to one of the
following supported MIME types:

* `image/png` (the default if none is given)
* `image/gif`
* `image/jpeg`
* `image/webp`

The desired DPI of the resulting image can be set through the query parameter
`dpi`. A value of `300` is suitable for most documents but can result in quite
large image file sizes.

cURL command to test the endpoint:

```bash
curl --data-binary "@my-file.pdf" \
  -H "Content-Type: application/pdf" \
  -H "Accept: image/webp" \
  -H "Authorization: token secret" \
  --output my-file.webp \
  "http://localhost:8507/?dpi=300"
```

### A note about multipage documents

PDF documents of multiple pages can be handled by `http-pdf-imager` in two ways.
The default operation is to convert all pages to images and then concatenate those
images vertically, stacked on top of each other.

Another option is to request a ZIP file to be returned, where each image version
of pages will still be separate files. Use the `Accept` header to enable ZIP,
like this:

```
Accept: image/jpeg,application/zip
```

## License

MIT
