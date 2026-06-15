#!/bin/sh
# Run the Dioxus fullstack server (handles /api/* server functions) in the
# background, then Caddy in the foreground. Caddy serves the static SSG site
# directly and reverse-proxies only /api/* to the server on 127.0.0.1:3000.
#
# Caddy is always the foreground process, so if the server binary is missing or
# crashes the SITE STAYS UP — /api just 502s. The server can never take the
# whole site down.
set -e

if [ -x /srvapp/server ]; then
  # The admin password travels base64-wrapped (PH_ADMIN_PASS_B64) so any chars
  # survive the deploy's JSON + shell quoting. Decode it just for the server.
  if [ -n "${PH_ADMIN_PASS_B64:-}" ]; then
    PH_ADMIN_PASS="$(printf '%s' "$PH_ADMIN_PASS_B64" | base64 -d 2>/dev/null || true)"
    export PH_ADMIN_PASS
  fi
  ( cd /srvapp && IP=127.0.0.1 PORT=3000 ./server ) &
  echo "started fullstack server on 127.0.0.1:3000"
else
  echo "WARN: /srvapp/server missing or not executable — /api disabled"
fi

exec caddy run --config /etc/caddy/Caddyfile --adapter caddyfile
