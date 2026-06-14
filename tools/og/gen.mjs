// Open Graph + repo social-card generator for Predator Hunters.
//
// Renders branded cards with the PH brand (ember gradient, self-hosted
// Fraunces/Hanken/Spline fonts, the wordmark) + an "Independent Press" tagline —
// NOT the research site's "Child Safety AI" card. Produces:
//   deploy/static/og.png                     the default site card (1200x630)
//   deploy/static/og/news/<slug>.png          one per published article (1200x630)
//   branding/social-ph-press.png              GitHub repo social preview (1280x640)
//   branding/social-ph-bulwark.png            GitHub repo social preview (1280x640)
//
// Article slug/title/kind are parsed straight from src/content.rs so the cards
// never drift. Run: `node tools/og/gen.mjs` (uses the puppeteer + chromium
// already installed in child-safety/tools/ui-tests).
import { createRequire } from "module";
import { readFileSync, mkdirSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, resolve } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, "..", "..");
const require = createRequire("C:/Users/Jordan/child-safety/tools/ui-tests/package.json");
const puppeteer = require("puppeteer");

const b64 = (p) => readFileSync(p).toString("base64");
const fontDir = resolve(ROOT, "deploy/static/fonts");
const FRAUNCES = b64(resolve(fontDir, "fraunces.woff2"));
const HANKEN = b64(resolve(fontDir, "hanken.woff2"));
const SPLINE = b64(resolve(fontDir, "spline.woff2"));
const LOGO = b64(resolve(ROOT, "assets/ph-logo.png"));

// --- parse src/content.rs: slug -> title -> kind, in struct field order ----
const src = readFileSync(resolve(ROOT, "src/content.rs"), "utf8");
const re = /slug:\s*"([^"]+)"[\s\S]*?title:\s*"([^"]+)"[\s\S]*?kind:\s*"([^"]+)"/g;
const articles = [];
let m;
while ((m = re.exec(src)) !== null) articles.push({ slug: m[1], title: m[2], kind: m[3] });
console.log(`parsed ${articles.length} articles from content.rs`);

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const headSize = (t, big) => {
  const k = big ? 1.12 : 1;
  return Math.round((t.length > 78 ? 46 : t.length > 56 ? 54 : t.length > 38 ? 62 : 72) * k);
};

function html({ eyebrow, headline, gradTail, sub, footer, tagline, w = 1200, h = 630, big = false }) {
  const hs = headSize(headline, big);
  let head = esc(headline);
  if (gradTail && head.endsWith(esc(gradTail))) {
    const stem = head.slice(0, head.length - esc(gradTail).length);
    head = `${stem}<span class="grad">${esc(gradTail)}</span>`;
  }
  return `<!doctype html><html><head><meta charset="utf-8"><style>
@font-face{font-family:'Fraunces';src:url(data:font/woff2;base64,${FRAUNCES}) format('woff2');font-weight:400 600;}
@font-face{font-family:'Hanken';src:url(data:font/woff2;base64,${HANKEN}) format('woff2');font-weight:400 700;}
@font-face{font-family:'Mono';src:url(data:font/woff2;base64,${SPLINE}) format('woff2');font-weight:400 600;}
*{margin:0;box-sizing:border-box;}
html,body{width:${w}px;height:${h}px;}
.card{position:relative;width:${w}px;height:${h}px;overflow:hidden;color:#F4F8FA;
  font-family:'Hanken',sans-serif;padding:${big ? 80 : 74}px ${big ? 88 : 82}px;display:flex;flex-direction:column;
  background:radial-gradient(58% 80% at 10% -6%,rgba(245,130,32,.20),transparent 60%),
             radial-gradient(52% 72% at 94% 4%,rgba(237,42,51,.17),transparent 58%),
             radial-gradient(60% 80% at 78% 116%,rgba(143,210,74,.10),transparent 64%),
             linear-gradient(158deg,#0B1622,#08111B);}
.grid{position:absolute;inset:0;opacity:.55;
  background-image:linear-gradient(rgba(176,205,224,.06) 1px,transparent 1px),
                   linear-gradient(90deg,rgba(176,205,224,.06) 1px,transparent 1px);
  background-size:64px 64px;
  -webkit-mask-image:radial-gradient(120% 82% at 50% 0%,#000,transparent 78%);}
.top{display:flex;align-items:center;gap:20px;position:relative;z-index:2;}
.logo{height:${big ? 50 : 44}px;display:block;filter:drop-shadow(0 2px 10px rgba(0,0,0,.5));}
.tagline{font-family:'Mono';font-size:15px;letter-spacing:.30em;text-transform:uppercase;
  color:#A6BAC8;padding-left:20px;border-left:1px solid rgba(176,205,224,.22);}
.mid{flex:1;display:flex;flex-direction:column;justify-content:center;position:relative;z-index:2;}
.eyebrow{font-family:'Mono';font-size:19px;letter-spacing:.26em;text-transform:uppercase;
  color:#8FD24A;margin-bottom:26px;display:flex;align-items:center;gap:14px;}
.eyebrow::before{content:"";width:34px;height:2px;background:linear-gradient(90deg,#8FD24A,transparent);}
.headline{font-family:'Fraunces';font-weight:400;font-size:${hs}px;line-height:1.05;
  letter-spacing:-.02em;color:#F4F8FA;max-width:20ch;}
.sub{margin-top:22px;font-size:21px;line-height:1.5;color:#A6BAC8;max-width:42ch;}
.grad{background:linear-gradient(98deg,#ED2A33,#F2592A 48%,#F58220);
  -webkit-background-clip:text;background-clip:text;color:transparent;font-style:italic;}
.bot{display:flex;align-items:center;justify-content:space-between;position:relative;z-index:2;}
.bot .url{font-family:'Mono';font-size:17px;letter-spacing:.14em;color:#6C8192;}
.bar{height:6px;width:128px;border-radius:99px;background:linear-gradient(98deg,#ED2A33,#F58220);
  box-shadow:0 8px 22px -8px rgba(245,130,32,.7);}
</style></head><body><div class="card">
  <div class="grid"></div>
  <div class="top"><img class="logo" src="data:image/png;base64,${LOGO}"><span class="tagline">${esc(tagline || "Independent Press")}</span></div>
  <div class="mid"><div class="eyebrow">${esc(eyebrow)}</div><div class="headline">${head}</div>${sub ? `<div class="sub">${esc(sub)}</div>` : ""}</div>
  <div class="bot"><span class="url">${esc(footer || "predatorhunters.co.uk")}</span><span class="bar"></span></div>
</div></body></html>`;
}

const cards = [
  {
    out: "deploy/static/og.png",
    eyebrow: "Independent local newsroom · since 2022",
    headline: "Local news, investigations, and the courts.",
    gradTail: "and the courts.",
  },
  ...articles.map((a) => ({
    out: `deploy/static/og/news/${a.slug}.png`,
    eyebrow: a.kind,
    headline: a.title,
  })),
  // GitHub repo social previews (1280x640).
  {
    out: "branding/social-ph-press.png",
    w: 1280,
    h: 640,
    big: true,
    eyebrow: "Independent local newsroom · since 2022",
    headline: "Predator Hunters",
    sub: "Local news and investigations, court reporting from the public record, reward appeals for information on serious crimes, and a public conviction database.",
    footer: "predatorhunters.co.uk",
  },
  {
    out: "branding/social-ph-bulwark.png",
    w: 1280,
    h: 640,
    big: true,
    tagline: "Child Safety",
    eyebrow: "PH Bulwark · child-safety",
    headline: "We protect children online.",
    gradTail: "online.",
    sub: "Consensual, guardian-installed content-filtering for a child's own device. Detect, block, report. Never store.",
    footer: "predatorhunters.co.uk",
  },
];

const browser = await puppeteer.launch({ headless: "new", args: ["--no-sandbox"] });
try {
  const page = await browser.newPage();
  for (const c of cards) {
    const w = c.w || 1200;
    const h = c.h || 630;
    await page.setViewport({ width: w, height: h, deviceScaleFactor: 1 });
    await page.setContent(html(c), { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    const outPath = resolve(ROOT, c.out);
    mkdirSync(dirname(outPath), { recursive: true });
    await page.screenshot({ path: outPath, type: "png", clip: { x: 0, y: 0, width: w, height: h } });
    console.log(`wrote ${c.out}  (${w}x${h}: ${c.headline})`);
  }
} finally {
  await browser.close();
}
console.log("cards done.");
