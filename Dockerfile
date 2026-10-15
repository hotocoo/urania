# syntax=docker/dockerfile:1
# Builds a fully static musl binary on the build platform and ships it in a
# scratch image. No cross-compilation: docker build produces a binary for
# the host architecture (use the GitHub Releases binaries for other arches).
FROM --platform=$BUILDPLATFORM rust:1.98 AS build
WORKDIR /src
# musl-tools provides the native musl linker for static builds.
RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/*
COPY . .
ARG BUILDPLATFORM
RUN case "$BUILDPLATFORM" in \
      linux/amd64) export TARGET=x86_64-unknown-linux-musl ;; \
      linux/arm64) export TARGET=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported platform: $BUILDPLATFORM" && exit 1 ;; \
    esac && rustup target add "$TARGET" && \
    cargo build --release --target "$TARGET"

FROM scratch
# Self-contained: TLS roots are compiled in (reqwest rustls/webpki-roots) and
# timestamps render as UTC, so the scratch image needs no CA store or
# zoneinfo. Mount a volume at /root/.local/share/urania for the database.
COPY --from=build /src/target/*/release/urania /urania
ENTRYPOINT ["/urania"]
CMD ["run", "--foreground"]
