# Build stage
FROM public.ecr.aws/docker/library/rust:bookworm AS builder
COPY . .
RUN cargo build --release

# Runtime stage
FROM public.ecr.aws/docker/library/debian:bookworm-slim

# Install runtime dependencies and CA certificates
RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssl \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && update-ca-certificates

COPY --from=builder /target/release/load-tester /app/load-tester

# Create a non-root user for security
RUN useradd -r -s /bin/false load-tester && \
    chown load-tester:load-tester /app/load-tester

USER load-tester
ENTRYPOINT ["/app/load-tester"]
CMD ["--users", "10", "--concurrent", "5"]