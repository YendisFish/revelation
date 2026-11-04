FROM rust:alpine

RUN apk update && apk add --no-cache \
    bash \
    musl-dev \
    gcc \
    make \
    libseccomp-dev \
    libseccomp-static \
    e2fsprogs \
    e2tools \
    mtools \
    dosfstools \
    cpio \
    gzip

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /bld/
COPY . .

WORKDIR /app
ENTRYPOINT [ "/bin/bash", "/bld/container.sh" ]
