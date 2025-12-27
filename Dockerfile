# Build stage
FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app

# Copy manifests only (not Cargo.lock - let Docker regenerate it for compatibility)
COPY Cargo.toml ./

# Create dummy main.rs for dependency caching
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (will generate fresh Cargo.lock)
RUN cargo build --release && rm -rf src

# Copy actual source code
COPY src ./src
COPY migrations ./migrations

# Build the application
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM alpine:3.19

RUN apk add --no-cache ca-certificates curl

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/transaction-service /app/transaction-service
COPY --from=builder /app/migrations /app/migrations

# Create non-root user
RUN addgroup -g 1000 app && adduser -u 1000 -G app -D app
USER app

EXPOSE 8080

ENV RUST_LOG=info

CMD ["/app/transaction-service"]
