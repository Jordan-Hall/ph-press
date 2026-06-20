// Diagnostic e2e: log into /desk, open the article editor, and inspect the
// Visual/Markdown toggle — capturing console errors (wasm panics log here),
// the DOM state of each editor container, and screenshots at each step.
//
//   node editor-check.mjs [baseUrl] [user] [pass]
// defaults: https://predatorhunters.co.uk  admin  PH-med!a1
import puppeteer from "puppeteer";
import { writeFileSync } from "node:fs";

const BASE = process.argv[2] || "https://predatorhunters.co.uk";
const USER = process.argv[3] || "admin";
const PASS = process.argv[4] || "PH-med!a1";

const logs = [];
const log = (...a) => { const s = a.join(" "); logs.push(s); console.log(s); };

const browser = await puppeteer.launch({ headless: "new", args: ["--no-sandbox"] });
const page = await browser.newPage();
await page.setViewport({ width: 1280, height: 900 });

page.on("console", (m) => log(`  [console.${m.type()}] ${m.text()}`));
page.on("pageerror", (e) => log(`  [PAGEERROR] ${e.message}`));
page.on("requestfailed", (r) => log(`  [reqfailed] ${r.url()} ${r.failure()?.errorText || ""}`));

const clickByText = (sel, text) =>
  page.evaluate((sel, text) => {
    const el = [...document.querySelectorAll(sel)].find((e) => e.textContent.trim().includes(text));
    if (el) { el.click(); return true; }
    return false;
  }, sel, text);

const editorState = () =>
  page.evaluate(() => {
    const rich = document.querySelector(".editor-rich");
    const taino = document.querySelector(".taino-editor");
    const split = document.querySelector(".editor-split");
    const ta = document.querySelector("#ed-body");
    const vis = (el) => { if (!el) return "absent"; const s = getComputedStyle(el); return s.display === "none" ? "hidden" : "shown"; };
    return {
      tabs: [...document.querySelectorAll(".em-tab")].map((b) => ({ t: b.textContent.trim(), on: b.className.includes("on") })),
      richWrapper: vis(rich),
      taino: taino ? { mounted: taino.childElementCount > 0, children: taino.childElementCount, editable: taino.getAttribute("contenteditable"), html: taino.innerHTML.slice(0, 160) } : "absent",
      markdownSplit: vis(split),
      textarea: ta ? vis(ta) : "absent",
      edBodyValue: ta ? ta.value : null,
      preview: document.querySelector(".editor-preview")?.innerHTML?.replace(/\s+/g, " ").slice(0, 1200) ?? null,
    };
  });

try {
  log(`\n=== 1. LOAD ${BASE}/desk ===`);
  await page.goto(`${BASE}/desk`, { waitUntil: "networkidle2", timeout: 60000 });
  await page.waitForSelector('input[type="password"]', { timeout: 30000 }).catch(() => log("  (no password field — already logged in?)"));
  await page.screenshot({ path: "shot-1-login.png" });

  const hasLogin = await page.$('input[type="password"]');
  if (hasLogin) {
    log(`=== 2. LOGIN as ${USER} ===`);
    await page.type('input[type="text"]', USER);
    await page.type('input[type="password"]', PASS);
    await clickByText("button", "Sign in");
    await new Promise((r) => setTimeout(r, 4000));
    await page.screenshot({ path: "shot-2-after-login.png" });
    if (await page.$('input[type="password"]')) {
      log("  !! STILL on login form — credentials likely wrong (PH_ADMIN_PASS set in deploy?). Cannot reach editor.");
      throw new Error("login-failed");
    }
    log("  logged in.");
  }

  log("=== 3. OPEN EDITOR (Write a story → /desk/edit/0) ===");
  if (!(await clickByText("a", "Write a story"))) {
    await page.goto(`${BASE}/desk/edit/0`, { waitUntil: "networkidle2", timeout: 60000 });
  }
  await new Promise((r) => setTimeout(r, 4000));
  await page.waitForSelector(".editor", { timeout: 20000 }).catch(() => log("  (no .editor form found)"));
  await page.screenshot({ path: "shot-3-editor-default.png" });
  log("  editor state (default / Visual): " + JSON.stringify(await editorState(), null, 2));

  log("=== 4. TOGGLE → Markdown ===");
  await clickByText("button", "Markdown");
  await new Promise((r) => setTimeout(r, 1500));
  await page.screenshot({ path: "shot-4-markdown.png" });
  log("  editor state (Markdown): " + JSON.stringify(await editorState(), null, 2));

  log("=== 4b. TYPE markdown → dioxus-markdown preview should render it ===");
  await page.evaluate(() => {
    const t = document.querySelector("#ed-body");
    if (t) {
      t.value = "## Heading\n\n**bold** and *italic* and a [link](https://example.com)\n\n- one\n- two";
      t.dispatchEvent(new Event("input", { bubbles: true }));
    }
  });
  await new Promise((r) => setTimeout(r, 1500));
  await page.screenshot({ path: "shot-4b-markdown-typed.png" });
  const md = await editorState();
  log("  editor state (Markdown + typed): " + JSON.stringify(md, null, 2));
  const p = md.preview || "";
  const ok = /<h[12]\b/.test(p) && /<(b|strong)\b/.test(p) && /<(a |li\b)/.test(p);
  log(`  >> dioxus-markdown rendered heading + bold + link/list? ${ok ? "YES ✓" : "NO ✗"}`);

  log("=== 5. TOGGLE → Visual ===");
  await clickByText("button", "Visual");
  await new Promise((r) => setTimeout(r, 1500));
  await page.screenshot({ path: "shot-5-visual-again.png" });
  log("  editor state (Visual again): " + JSON.stringify(await editorState(), null, 2));

  log("=== 6. TYPE in Visual editor ===");
  await page.evaluate(() => { const t = document.querySelector(".taino-editor"); if (t) t.focus(); });
  await page.keyboard.type("Hello newsroom").catch((e) => log("  type failed: " + e.message));
  await new Promise((r) => setTimeout(r, 1000));
  await page.screenshot({ path: "shot-6-typed.png" });
  log("  editor state (after typing): " + JSON.stringify(await editorState(), null, 2));
} catch (e) {
  log(`\n!! ABORTED: ${e.message}`);
} finally {
  writeFileSync("editor-check.log", logs.join("\n"));
  await browser.close();
  log("\n=== done. screenshots: shot-*.png, log: editor-check.log ===");
}
