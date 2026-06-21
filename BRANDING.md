# Rebranding a fork

The public site is **pre-rendered (SSG) at build time**, so the brand is baked into
the static HTML when CI builds the image. To re-skin a fork, edit the few build-time
sources below, then rebuild + redeploy (`dx build --fullstack --ssg` / push to
`master`). Nothing is configured at runtime.

## 1. Name, tagline, URL, contact emails — `src/config.rs`

The newsroom identity as build-time constants, each also overridable via an
environment variable at build time:

| Const | Env override | Used for |
|---|---|---|
| `SITE_NAME` | `PH_SITE_NAME` | masthead, page titles, OG `site_name` |
| `TAGLINE` | `PH_TAGLINE` | strapline under the masthead |
| `BASE_URL` | `PH_BASE_URL` | canonical + OG URLs (no trailing slash) |
| `TIPS_EMAIL` / `PRESS_EMAIL` / `COMPLAINTS_EMAIL` | `PH_TIPS_EMAIL` / `PH_PRESS_EMAIL` / `PH_COMPLAINTS_EMAIL` | contact lanes |

Either edit the defaults in `src/config.rs`, or pass the env vars at build time:

```
PH_SITE_NAME="Acme Watch" PH_BASE_URL="https://acmewatch.org" \
  dx build --fullstack --ssg --release
```

## 2. Colours — `index.html` `:root`

Under the **`BRAND PALETTE`** comment in the `:root { … }` block. Change the core
tokens AND their `html[data-theme="light"]` overrides:

- `--paper` / `--paper-2` / `--sunk` — backgrounds
- `--ink` / `--ink-2` — text
- `--red` / `--red-2` / `--on-red` — accent
- `--tag` — accent tint
- `--serif` / `--sans` / `--mono` — font stacks

Everything else in `:root` derives from these.

## 3. Logo + favicon — `assets/` (via `src/assets.rs`)

Replace the files referenced by `PH_LOGO` / `FAVICON` in `src/assets.rs` — by
default `assets/ph-logo.png` and `assets/favicon.png` — with your own, keeping the
same paths (or update the paths in `src/assets.rs`).

## 4. Editorial copy

The org's story and standing-page prose live in `src/content.rs` and the page
components under `src/pages/`; a fork rewrites those directly.

---

Then rebuild and redeploy — that's the entire brand surface. The first run of a
fresh deploy opens a one-time **"Set up the newsroom"** screen at `/desk` to create
the first administrator (no default password).
