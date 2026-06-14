# predatorhunters.co.uk — deploy

The Predator Hunters main site (Dioxus 0.8 web/wasm SSG, this repo) ships as a
small Caddy image, built in CI and run on the existing EC2 box via AWS SSM — the
same no-SSH pattern as the research site and the bulwark server.

## Architecture — shared edge on one box

The box already runs the **research-site** container (owns host `:80`/`:443`, holds
the Let's Encrypt cert) and **bulwark-server** (`:8443`). `predatorhunters.co.uk`
and `research.predatorhunters.co.uk` are the same Cloudflare zone (one SSL mode),
and two containers cannot both bind `:443`, so:

- **This container** serves the SSG site over **plain HTTP on `127.0.0.1:8090`**
  (loopback-only — not exposed to the network) and reverse-proxies `/api/*` to the
  Dioxus fullstack server beside it. No TLS, no cert state, no Cloudflare token.
- **The research-site container is the shared edge**: its Caddyfile (in the
  `child-safety` repo, `deploy/research/Caddyfile`) has a `predatorhunters.co.uk`
  block that terminates TLS (LE cert via Cloudflare DNS-01) and reverse-proxies the
  apex to `127.0.0.1:8090`. It adds HSTS; this container owns the app CSP/headers.

```
Cloudflare (proxied, Full strict)
        │
   research-site container ── edge Caddy :443
        ├── research.predatorhunters.co.uk → /srv (local)
        └── predatorhunters.co.uk          → 127.0.0.1:8090
                                                  │
                                            ph-press container
                                            Caddy :8090 (plain HTTP)
                                            ├── /srv  static SSG
                                            └── /api  → 127.0.0.1:3000 (fullstack server)
```

## Pipeline

`.github/workflows/site.yml` (on push to `master` touching `src/**`, `content/**`,
`deploy/**`, `assets/**`, `Cargo.*`, `index.html`, or the workflow; or **Run workflow**):

1. **build-image** — `docker build -f deploy/Dockerfile .` runs
   `dx build --fullstack --ssg --release` (pre-renders every route incl. each
   `/news/:slug`), bakes the bundle + a stock Caddy into `debian-slim`,
   smoke-tests it, and pushes `ghcr.io/jordan-hall/ph-press:{sha,latest}`.
2. **deploy** — SSM `docker pull` + `docker run -d --name ph-press -p 127.0.0.1:8090:8090
   -v /var/lib/ph-press:/data` on the box (Environment `production`), then checks
   `/healthz`.

## One-time prerequisites (outside CI)

1. **This repo's GitHub config** — create the `production` **Environment** and add:
   - secrets `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` (the scoped
     `ph-bulwark-deployer`, `ssm:SendCommand` only — same values as the other repos)
   - vars `AWS_REGION` (`eu-west-2`), `AWS_INSTANCE_ID` (`i-0a3aa9dc27f8e1c91`)
2. **Cloudflare DNS** — **proxied** (orange-cloud) `A` records `@` (apex) and `www`
   → `35.179.110.106`. Same zone/SSL-mode as the research record (Full strict).
3. **Edge routing** — the `predatorhunters.co.uk` block in
   `child-safety:deploy/research/Caddyfile`, deployed via the research-site workflow.
   (Ships alongside this; the apex serves once the research container redeploys.)
4. **Security group** — no change. `:443` is already open for research; `:8090` is
   loopback-only.

No `BULWARK_CF_DNS_TOKEN` is needed here (the edge container holds it for the apex cert).
