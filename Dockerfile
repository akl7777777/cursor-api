ARG TARGETARCH
FROM --platform=linux/${TARGETARCH} rust:1-slim-bookworm as builder

ARG TARGETARCH

WORKDIR /app
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential protobuf-compiler pkg-config libssl-dev nodejs npm openssl \
    && rm -rf /var/lib/apt/lists/*

COPY . .
RUN case "$TARGETARCH" in amd64) TARGET_CPU="x86-64-v3" ;; arm64) TARGET_CPU="neoverse-n1" ;; *) echo "Unsupported architecture: $TARGETARCH" && exit 1 ;; esac && RUSTFLAGS="-C link-arg=-s -C target-cpu=$TARGET_CPU" cargo build --release && cp target/release/cursor-api /app/cursor-api

# 运行阶段
FROM --platform=linux/${TARGETARCH} debian:bookworm-slim

WORKDIR /app
ENV TZ=Asia/Shanghai

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates tzdata openssl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/cursor-api .

ENV PORT=3000
EXPOSE ${PORT}
CMD ["./cursor-api"]