# Predator Hunters — the main site (PH Press)

The public website for **Predator Hunters** ([predatorhunters.co.uk](https://predatorhunters.co.uk)):
independent child-protection and court-reporting journalism. Online decoy operations,
court reporting **from the public record** (post-conviction only), a public conviction
database, and the editorial standards you can hold us to.

> We never name anyone before they are charged. We hold footage back until there is a
> conviction, censor it where needed, and only publish it where it helps keep children safe.

Sister site: the research / technology arm at
[research.predatorhunters.co.uk](https://research.predatorhunters.co.uk).

## Stack

- **[Dioxus 0.8](https://dioxuslabs.com/) (Rust → WebAssembly)**, one component tree.
- **Web = SSG** (`dx build --fullstack --ssg`): every route — including each
  `/news/:slug` — is pre-rendered to real HTML so crawlers, link bots and no-JS
  clients get the full page, then the wasm hydrates. Essential for a newsroom.
- A small **fullstack `server`** binary handles `/api/*` server functions.
- Self-hosted fonts + inlined CSS (no third-party requests; no visitor-IP leak).
- Native Android/iOS/desktop from the same components (dioxus-native, experimental) — WS4.

## Layout

| Path | What |
|---|---|
| `src/app.rs` | Router (`Routable`) + persistent shell (nav/footer/theme) + the SSG route hook |
| `src/pages/` | One component per route: home, news, `news/:slug`, database, cases, watch, podcast, about, standards, contact, privacy |
| `src/content.rs` | Compile-time article store (until the CMS lands); drives `/news` + per-article OG |
| `src/components.rs` | `Seo`, footer, closing CTA |
| `tools/og/gen.mjs` | Branded OG / repo social-card generator (puppeteer) |
| `deploy/` | Docker + Caddy + CI ([deploy/README.md](deploy/README.md)) |

## Develop

```sh
dx serve --platform web          # dev server + hot reload at http://localhost:8080
dx build --fullstack --ssg --release   # production SSG build (the CI gate)
node tools/og/gen.mjs            # regenerate OG cards after changing articles
```

## Deploy

Auto-deploys on push to `master` via GitHub Actions → GHCR image → AWS SSM `docker run`
on the shared EC2 box, behind Cloudflare. It runs as its own container alongside the
research site (which fronts `:443` as the shared edge). Full details + one-time setup in
**[deploy/README.md](deploy/README.md)**.

## Standards

We are building towards registration with **[IMPRESS](https://www.impressorg.com/)**, the
UK's approved press regulator. Accuracy, corrections, a complaints process, an
active-proceedings gate (Contempt of Court Act 1981) and transparency are built into the
editorial workflow. See [`/standards`](https://predatorhunters.co.uk/standards).

## Licence

© Predator Hunters. All rights reserved. Public for transparency and free CI; not an
invitation to reuse the brand, content, or reporting.
