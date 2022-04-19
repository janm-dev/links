# Builder image
FROM rust:latest AS builder

# Install musl libc for static linking and cmake to build .proto files for gRPC
RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev cmake
RUN update-ca-certificates

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

# Final image
FROM scratch

# Import from builder
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /links

# Copy the build
COPY --from=builder /links/target/x86_64-unknown-linux-musl/release/server ./

# Use an unprivileged user
USER links:links

# Expose all usual ports (80 for HTTP, 530 for gRPC)
EXPOSE 80
EXPOSE 530

ENTRYPOINT [ "/links/server" ]
