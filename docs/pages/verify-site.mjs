import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const docs = resolve(here, "..");
const origin = "https://liuchong.github.io/fighorse/";

const pages = [
  { file: resolve(docs, "index.html"), canonical: origin },
  { file: resolve(here, "capabilities.html"), canonical: `${origin}pages/capabilities.html` },
  { file: resolve(here, "workflow.html"), canonical: `${origin}pages/workflow.html` },
  { file: resolve(here, "architecture.html"), canonical: `${origin}pages/architecture.html` },
  { file: resolve(here, "getting-started.html"), canonical: `${origin}pages/getting-started.html` },
];

const titles = new Set();
const descriptions = new Set();

function match(html, expression, message) {
  const result = html.match(expression);
  assert.ok(result, message);
  return result;
}

for (const page of pages) {
  assert.ok(existsSync(page.file), `Missing required page: ${page.file}`);
  const html = readFileSync(page.file, "utf8");
  const title = match(html, /<title>([^<]+)<\/title>/i, `${page.file} needs a title`)[1].trim();
  const description = match(
    html,
    /<meta\s+name="description"\s+content="([^"]+)"/i,
    `${page.file} needs a meta description`,
  )[1].trim();

  assert.ok(title.length >= 12 && title.length <= 70, `${page.file} title length is unsuitable`);
  assert.ok(description.length >= 40 && description.length <= 180, `${page.file} description length is unsuitable`);
  assert.ok(!titles.has(title), `${page.file} title must be unique`);
  assert.ok(!descriptions.has(description), `${page.file} description must be unique`);
  titles.add(title);
  descriptions.add(description);

  assert.match(html, /<html\s+lang="zh-CN"/i, `${page.file} needs zh-CN language metadata`);
  assert.match(html, /<meta\s+name="viewport"/i, `${page.file} needs a viewport`);
  assert.match(html, new RegExp(`<link rel="canonical" href="${page.canonical.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"`), `${page.file} canonical URL is wrong`);
  assert.match(html, /<meta\s+property="og:type"/i, `${page.file} needs Open Graph metadata`);
  assert.match(html, /<meta\s+property="og:image"\s+content="https:\/\/liuchong\.github\.io\/fighorse\/pages\/social-preview\.png"/i, `${page.file} needs the social card`);
  assert.match(html, /<meta\s+name="twitter:card"\s+content="summary_large_image"/i, `${page.file} needs X/Twitter metadata`);
  assert.match(html, /<link\s+rel="sitemap"/i, `${page.file} needs a sitemap link`);
  assert.match(html, /<link\s+rel="icon"[^>]+logo\.svg/i, `${page.file} needs the official horse logo favicon`);
  assert.match(html, /<script\s+type="application\/ld\+json">[\s\S]*?<\/script>/i, `${page.file} needs JSON-LD`);
  assert.match(html, /<a\s+class="skip-link"\s+href="#main-content"/i, `${page.file} needs a skip link`);
  assert.match(html, /<main\b[^>]*\bid="main-content"/i, `${page.file} needs a main landmark`);
  assert.equal((html.match(/<h1(?:\s|>)/gi) || []).length, 1, `${page.file} needs exactly one h1`);
  assert.match(html, /<nav[^>]+aria-label="主导航"/i, `${page.file} needs an accessible primary nav`);
  assert.doesNotMatch(html, /<script[^>]+src="https?:\/\//i, `${page.file} must not load third-party scripts`);
  assert.doesNotMatch(html, /<link[^>]+rel="stylesheet"[^>]+href="https?:\/\//i, `${page.file} must not load third-party styles`);

  for (const block of html.matchAll(/<script\s+type="application\/ld\+json">([\s\S]*?)<\/script>/gi)) {
    assert.doesNotThrow(() => JSON.parse(block[1]), `${page.file} has invalid JSON-LD`);
  }

  const sourceDir = dirname(page.file);
  for (const link of html.matchAll(/href="([^"]+)"/gi)) {
    const href = link[1];
    if (href.startsWith("#") || href.startsWith("http") || href.startsWith("mailto:")) continue;
    const path = href.split("#")[0].split("?")[0];
    if (!path || path.startsWith("data:")) continue;
    assert.ok(existsSync(resolve(sourceDir, path)), `${page.file} links to missing local file: ${href}`);
  }
}

for (const asset of ["styles.css", "site.js", "logo.svg", "social-preview.png", "site.webmanifest", "sitemap.xml", "robots.txt"]) {
  assert.ok(existsSync(resolve(here, asset)), `Missing site asset: ${asset}`);
}

const sitemap = readFileSync(resolve(here, "sitemap.xml"), "utf8");
for (const page of pages) assert.ok(sitemap.includes(`<loc>${page.canonical}</loc>`), `Sitemap is missing ${page.canonical}`);

const robots = readFileSync(resolve(here, "robots.txt"), "utf8");
assert.ok(robots.includes("Sitemap: https://liuchong.github.io/fighorse/pages/sitemap.xml"), "robots.txt must advertise the sitemap");

const manifest = JSON.parse(readFileSync(resolve(here, "site.webmanifest"), "utf8"));
assert.equal(manifest.name, "fighorse");
assert.equal(manifest.start_url, "../index.html");

const css = readFileSync(resolve(here, "styles.css"), "utf8");
const javascript = readFileSync(resolve(here, "site.js"), "utf8");
for (const brandColor of ["#172033", "#f7f3e8", "#ff6b6b", "#a678ff", "#29a9e8", "#31c991"]) {
  assert.ok(css.includes(brandColor), `Styles must use the official logo color ${brandColor}`);
}
assert.ok(!css.includes("#b8ff33"), "The first-version acid green must be replaced by the logo palette");
assert.match(css, /\.brand-logo\s*\{/, "Navigation must style the official horse logo");
assert.match(javascript, /document\.documentElement\.classList\.add\("js"\)/, "JavaScript must opt into enhanced navigation");
assert.match(css, /\.js\s+\.site-nav/, "The collapsed mobile navigation must only apply when JavaScript is available");
assert.match(css, /\.reveal\s*\{[^}]*opacity:\s*1/s, "Content must remain visible without JavaScript");
assert.match(css, /\.js\s+\.reveal\s*\{[^}]*opacity:\s*0/s, "Reveal animation must be a JavaScript-only enhancement");
assert.match(javascript, /replace\(\/\^\[\\t \]\*\\\$\[\\t \]\?\/gm,\s*""\)/, "Copied commands must remove shell prompt markers");

const architecture = readFileSync(resolve(here, "architecture.html"), "utf8");
assert.ok(!architecture.includes("四个开关"), "Security domains must not be described as four switches");
for (const gate of ["FIGHORSE_CANVAS_MODE", "FIGHORSE_CANVAS_SCRIPT", "yes=true"]) {
  assert.ok(architecture.includes(gate), `Architecture page must document the ${gate} canvas gate`);
}

console.log(`Verified ${pages.length} pages, shared assets, internal links, accessibility landmarks, and SEO metadata.`);
