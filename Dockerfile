##############################
## Build bcr-relay
##############################
FROM rust:latest AS rust-builder

WORKDIR /bcr-relay
RUN update-ca-certificates
COPY . .

RUN cargo build --release

##############################
## Create image for docker compose
##############################
FROM ubuntu:22.04

RUN apt-get update && \
  apt-get install -y ca-certificates libpq5 && \
  apt-get clean

WORKDIR /relay

# Copy binary release
COPY --from=rust-builder /bcr-relay/target/release/bcr-relay ./bcr-relay
COPY --from=rust-builder /bcr-relay/static/ ./static/

RUN chmod +x /relay/bcr-relay

# Expose server port
EXPOSE 8080

CMD ["/relay/bcr-relay", "--listen-address=0.0.0.0:8080"]