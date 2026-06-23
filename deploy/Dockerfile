# Predator Hunters (predatorhunters.co.uk) — build the Dioxus 0.8 fullstack
# bundle, then serve it with Caddy + the fullstack server beside it.
#
# Unlike the research site, this container does NOT terminate TLS. It serves the
# pre-rendered static SSG site over PLAIN HTTP on :8090 and reverse-proxies only
# /api/* to the fullstack server. The shared EDGE Caddy (the research-site
# container) owns :443, holds the Let's Encrypt cert for predatorhunters.co.uk,
# and reverse-proxies the apex to this container on 127.0.0.1:8090. So this image
# needs no Cloudflare DNS plugin and no cert state.
#
# Caddy is always the foreground process: if the fullstack server is missing or
# crashes, the SITE STAYS UP and /api just 502s. Runtime is debian-slim because
# the Rust server binary is glibc; the Caddy binary is static.
#
# Build context is the REPO ROOT (ph-press is a standalone single-crate repo).

# ---- 1. build the wasm bundle + fullstack server -------------------------
FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown
RUN cargo install dioxus-cli --version 0.8.0-alpha.0 --locked
WORKDIR /app
COPY . .
# Fullstack SSG: pre-render every static route (incl. each /news/:slug article)
# to real HTML for crawlers / no-JS, the wasm hydrates on the client, and the
# same build emits a `server` binary that handles /api/* server functions.
RUN dx build --fullstack --ssg --release
# The bundle is .../release/web/{server, public/, static/}. Merge public+static
# into /site for Caddy's static root, and stage the whole bundle (incl. the
# server binary) into /srvapp for the fullstack server process.
RUN set -eux; \
    WEB="$(dirname "$(find /app -type d -path '*release/web/public' | head -n1)")"; \
    test -n "$WEB"; test -d "$WEB/static"; \
    mkdir -p /site; \
    cp -r "$WEB/public/." /site/; \
    cp -r "$WEB/static/." /site/; \
    mkdir -p /srvapp; \
    cp -r "$WEB/." /srvapp/; \
    test -x /srvapp/server || { echo "ERROR: no executable server binary in bundle"; ls -la "$WEB"; exit 1; }; \
    echo "=== /site routes ==="; find /site -name index.html | sort; \
    echo "=== server binary ==="; ls -la /srvapp/server

# ---- 2. runtime ----------------------------------------------------------
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
# Stock Caddy (no DNS plugin needed — this container serves plain HTTP; the edge
# terminates TLS).
COPY --from=caddy:2 /usr/bin/caddy /usr/bin/caddy
# Litestream continuous SQLite → S3 backup. The static binary from the official
# litestream image is copied directly — no curl/tar/cleanup needed.
COPY --from=litestream/litestream:0.3.14 /usr/local/bin/litestream /usr/local/bin/litestream
COPY deploy/litestream.yml /etc/litestream.yml
COPY deploy/Caddyfile /etc/caddy/Caddyfile
COPY --from=build /site /srv
COPY --from=build /srvapp /srvapp
# Static SEO + share files at the root: /robots.txt, /sitemap.xml, /og.png,
# /apple-touch-icon.png, /fonts/*.woff2, /.well-known/security.txt.
COPY deploy/static/ /srv/
COPY deploy/entrypoint.sh /usr/local/bin/entrypoint.sh
# Strip any CR so a Windows-checkout CRLF shebang ("/bin/sh\r") doesn't fail to
# exec and take the container (and the site) down.
RUN sed -i 's/\r$//' /usr/local/bin/entrypoint.sh && chmod +x /usr/local/bin/entrypoint.sh
EXPOSE 8090
# CMD (not ENTRYPOINT) so the CI smoke test `docker run IMAGE caddy version`
# works by overriding the command.
CMD ["/usr/local/bin/entrypoint.sh"]
