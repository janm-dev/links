# Builder image
FROM rust:latest AS builder

# Install musl libc for static linking and openssl for default cert generation
RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt upgrade -y && apt install -y musl-tools musl-dev openssl
RUN update-ca-certificates

# Install protoc to build .proto files for gRPC
ENV PROTOC_VERSION=21.9
RUN curl -LO https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip
RUN unzip protoc-${PROTOC_VERSION}-linux-x86_64.zip -d /protoc/
ENV PROTOC="/protoc/bin/protoc"

# Create user
ENV USER=links
ENV UID=1001

RUN adduser \
	--disabled-password \
	--gecos "" \
	--home "/nonexistent" \
	--shell "/sbin/nologin" \
	--no-create-home \
	--uid "${UID}" \
	"${USER}"

WORKDIR /links

COPY ./ .

# Build with statically-linked musl libc
RUN cargo build --target x86_64-unknown-linux-musl --release

# Generate default TLS certificate
RUN openssl req -x509 -newkey rsa:4096 -sha256 -utf8 -days 3650 -nodes -config ./openssl.conf -keyout /key.pem -out /cert.pem

# Final image
FROM scratch

# Import from builder
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
COPY --from=builder --chown=links:links /cert.pem /cert.pem
COPY --from=builder --chown=links:links /key.pem /key.pem

WORKDIR /links

# Copy the build
COPY --from=builder /links/target/x86_64-unknown-linux-musl/release/server ./

# Copy the config file
COPY ./docker-config.toml /config.toml

# Use an unprivileged user
USER links:links

# Expose all usual ports (80 for HTTP, 443 for HTTPS, 530 for gRPCS)
EXPOSE 80
EXPOSE 443
EXPOSE 530

ENTRYPOINT [ "/links/server" ]
CMD [ "--config", "/config.toml" ]
