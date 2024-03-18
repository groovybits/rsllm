FROM rust:1.76.0 as builder
RUN apt-get update && apt-get -y install cmake libpcap-dev cabextract

WORKDIR /app
COPY src/ src/
COPY Cargo.toml .
COPY fonts/ fonts/
COPY scripts/ scripts/
COPY libndi.dylib .

RUN cargo build --release
RUN cargo install --path .
RUN fonts/unpack_fonts.sh

FROM cgr.dev/chainguard/wolfi-base AS binary
COPY --from=builder /usr/local/cargo/bin/rsllm /usr/local/bin/rsllm
RUN apk update && apk add --no-cache --update-cache libgcc libpcap libstdc++

ARG SOURCE_DEVICE=eth0
ARG TARGET_IP=127.0.0.1
ARG TARGET_PORT=5556

ENV SOURCE_DEVICE=${SOURCE_DEVICE}
ENV TARGET_IP=${TARGET_IP}
ENV TARGET_PORT=${TARGET_PORT}
ENV RUST_LOG="info"

ENTRYPOINT ["rsllm"]
