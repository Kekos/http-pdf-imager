#!/usr/bin/env bash

set -e

if [[ $TARGETARCH = "amd64" ]]; then
  TARGETARCH="x64"
fi

if [[ $TARGETARCH = "" ]]; then
  echo "You must set the TARGETARCH environment variable."
  exit 1
fi

echo $TARGETARCH

PDFIUM_URL="https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-$TARGETARCH.tgz"

echo $PDFIUM_URL

curl --location "$PDFIUM_URL" | tar zx
