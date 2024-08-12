FROM rust:1 AS build-env

ARG TARGETARCH

WORKDIR /app
COPY ./src /app/src
COPY ./Cargo.lock /app
COPY ./Cargo.toml /app
RUN cargo build --release

WORKDIR /pdfium
COPY ./download_pdfium.sh /pdfium
RUN ./download_pdfium.sh

FROM gcr.io/distroless/cc-debian12

ENV HPI_PORT=8507

COPY --from=build-env /app/target/release/http-pdf-imager /
COPY --from=build-env --chown=root:root /pdfium/lib/libpdfium.so /lib
COPY --from=build-env --chown=root:root /pdfium/LICENSE /lib/libpdfium_LICENSE

USER nobody
ENTRYPOINT ["./http-pdf-imager"]
EXPOSE ${HPI_PORT}
