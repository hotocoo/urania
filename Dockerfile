# syntax=docker/dockerfile:1
# Builds a fully static musl binary on the build platform and ships it in a
# scratch image. No cross-compilation: docker build produces a binary for
# the host architecture (use the GitHub Releases binaries for other arches).
# Builder-portable: no $BUILDPLATFORM (BuildKit-only), works with the
# legacy builder too.
FROM rust:1.98 AS build
WORKDIR /src
# musl-tools provides the native musl linker for static builds.
RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN ARCH=$(uname -m) && case "$ARCH" in \
      x86_64) export TARGET=x86_64-unknown-linux-musl ;; \
      aarch64) export TARGET=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported arch: $ARCH" && exit 1 ;; \
    esac && rustup target add "$TARGET" && \
    cargo build --release --target "$TARGET"

FROM scratch
# Self-contained: TLS roots are compiled in (reqwest rustls/webpki-roots) and
# timestamps render as UTC, so the scratch image needs no CA store or
# zoneinfo. Mount a volume at /root/.local/share/urania for the database.
# HOME must be set: with it unset the data dir falls back to a
# CWD-relative path and the documented volume mount is silently unused.
ENV HOME=/root
COPY --from=build /src/target/*/release/urania /urania
ENTRYPOINT ["/urania"]
CMD ["run", "--foreground"]
