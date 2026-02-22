###############################################################################
## Builder
###############################################################################
FROM rust:alpine3.23 AS builder
RUN apk add --no-cache \
            musl-dev \
            pkgconfig \
            gcc \
            make && \
    rm -rf /var/cache/apk

WORKDIR /builder

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

COPY src ./src

RUN cargo build --release --locked

###############################################################################
## Final image
###############################################################################
FROM alpine:3.23

RUN apk add --update --no-cache \
    ca-certificates && \
    rm -rf /var/cache/apk

ENV USER=app \
    UID=1000

WORKDIR /app

COPY --from=builder /builder/target/release/populatrs /usr/local/bin/populatrs

# Create the user
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/${USER}" \
    --shell "/sbin/nologin" \
    --uid "${UID}" \
    "${USER}" && \
    mkdir -p /app/data && \
    chown -R app:app /app

USER app
# Set environment variables
ENV RUST_LOG=info
# Expose volume for data persistence
VOLUME ["/app/data"]
# Default command
CMD ["populatrs", "--config", "/app/config.json"]
