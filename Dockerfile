# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM rust:1-stable-musl AS build
WORKDIR /src
COPY . .
ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
      x86_64) export TARGET=x86_64-unknown-linux-musl ;; \
      arm64)  export TARGET=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported platform: $TARGETPLATFORM" && exit 1 ;; \
    esac && rustup target add "$TARGET" && \
    cargo build --release --target "$TARGET"

FROM scratch
# rustls needs the web PKI root store; zoneinfo for local-time rendering.
COPY --from=build /etc/ssl/certs /etc/ssl/certs
COPY --from=build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=build /src/target/*/release/urania /urania
# The container's /etc/resolv.conf (mounted by the runtime) is the DNS
# measurement target for the dns probe; the report header records the
# resolver provenance so the evidence states what was measured.
ENTRYPOINT ["/urania"]
CMD ["run", "--foreground"]
