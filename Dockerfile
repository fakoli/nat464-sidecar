FROM rust:1.84-slim AS builder

WORKDIR /app

# Cache dependencies by building a dummy project first
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Build the real project
COPY src/ src/
RUN touch src/main.rs && cargo build --release

# Runtime: minimal distroless image
FROM gcr.io/distroless/cc-debian12

COPY --from=builder /app/target/release/nat464-sidecar /nat464-sidecar

USER nonroot:nonroot

ENTRYPOINT ["/nat464-sidecar"]
