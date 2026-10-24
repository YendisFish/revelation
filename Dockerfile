FROM rust:alpine

RUN apk update && apk add bash musl-dev gcc make libseccomp-dev libseccomp-static
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /bld/
COPY . .

WORKDIR /app
ENTRYPOINT [ "/bin/bash", "/bld/container.sh" ]
