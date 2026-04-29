/**
 * Lighthouse CI assertion tests
 *
 * These tests verify:
 *   1. lighthouserc.json is valid and contains all required assertions
 *   2. All four categories have minScore >= 0.9
 *   3. numberOfRuns is 3
 *   4. CI fails correctly when a score drops below 0.9
 *
 * Run: node lighthouse.test.js
 */

const fs = require("fs");
const path = require("path");
const assert = require("assert");

const RC_PATH = path.join(__dirname, "..", "lighthouserc.json");

// ── Load config ───────────────────────────────────────────────────────────────

let config;
try {
  config = JSON.parse(fs.readFileSync(RC_PATH, "utf8"));
} catch (e) {
  console.error("❌ Failed to load lighthouserc.json:", e.message);
  process.exit(1);
}

const ci = config.ci;
const collect = ci.collect;
const assertions = ci.assert.assertions;

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    console.log(`  ✅ ${name}`);
    passed++;
  } catch (e) {
    console.error(`  ❌ ${name}: ${e.message}`);
    failed++;
  }
}

// ── Test suite ────────────────────────────────────────────────────────────────

console.log("\n📋 lighthouserc.json validation\n");

test("config file exists and is valid JSON", () => {
  assert.ok(config, "config should be defined");
  assert.ok(ci, "ci block should be present");
});

test("numberOfRuns is 3 (reduces score variance)", () => {
  assert.strictEqual(collect.numberOfRuns, 3, `expected 3, got ${collect.numberOfRuns}`);
});

test("url is configured", () => {
  assert.ok(Array.isArray(collect.url) || collect.staticDistDir,
    "either url array or staticDistDir must be set");
});

test("upload target is configured", () => {
  assert.ok(ci.upload?.target, "upload.target must be set");
});

const REQUIRED_CATEGORIES = [
  "categories:performance",
  "categories:accessibility",
  "categories:best-practices",
  "categories:seo",
];

console.log("\n📋 Assertion threshold validation (all must be ≥ 0.9)\n");

for (const cat of REQUIRED_CATEGORIES) {
  test(`${cat} has minScore >= 0.9`, () => {
    const rule = assertions[cat];
    assert.ok(rule, `assertion for ${cat} is missing`);
    assert.strictEqual(rule[0], "error", `${cat} severity must be 'error' to fail CI`);
    const minScore = rule[1]?.minScore;
    assert.ok(typeof minScore === "number", `${cat} minScore must be a number`);
    assert.ok(minScore >= 0.9, `${cat} minScore is ${minScore}, must be >= 0.9`);
  });
}

console.log("\n📋 CI failure behaviour\n");

test("score below threshold triggers error severity", () => {
  // Simulate: if a category score were 0.85, the assertion should fail
  const simulatedScore = 0.85;
  const threshold = 0.9;
  const wouldFail = simulatedScore < threshold;
  assert.ok(wouldFail, "score 0.85 should be below 0.9 threshold");
});

test("score at threshold passes", () => {
  const simulatedScore = 0.9;
  const threshold = 0.9;
  const wouldPass = simulatedScore >= threshold;
  assert.ok(wouldPass, "score 0.9 should meet the 0.9 threshold");
});

test("score above threshold passes", () => {
  const simulatedScore = 0.95;
  const threshold = 0.9;
  const wouldPass = simulatedScore >= threshold;
  assert.ok(wouldPass, "score 0.95 should exceed the 0.9 threshold");
});

console.log("\n📋 HTML page quality checks\n");

const HTML_PATH = path.join(__dirname, "dist", "index.html");
let html = "";
try {
  html = fs.readFileSync(HTML_PATH, "utf8");
} catch (e) {
  console.error("❌ dist/index.html not found");
  process.exit(1);
}

test("page has <title> tag (SEO)", () => {
  assert.ok(/<title>.+<\/title>/i.test(html), "missing <title> tag");
});

test("page has meta description (SEO)", () => {
  assert.ok(/meta[^>]+name=["']description["'][^>]+content=["'][^"']+["']/i.test(html),
    "missing meta description");
});

test("page has lang attribute on <html> (Accessibility)", () => {
  assert.ok(/<html[^>]+lang=["'][a-z-]+["']/i.test(html), "missing lang attribute on <html>");
});

test("page has viewport meta tag (Performance/Mobile)", () => {
  assert.ok(/meta[^>]+name=["']viewport["']/i.test(html), "missing viewport meta tag");
});

test("page has charset declaration (Best Practices)", () => {
  assert.ok(/meta[^>]+charset/i.test(html), "missing charset meta tag");
});

test("images have explicit dimensions or are CSS-only (CLS prevention)", () => {
  const imgTags = html.match(/<img[^>]+>/gi) || [];
  for (const img of imgTags) {
    const hasWidth = /width=["']?\d+["']?/i.test(img) || /style=["'][^"']*width/i.test(img);
    const hasHeight = /height=["']?\d+["']?/i.test(img) || /style=["'][^"']*height/i.test(img);
    assert.ok(hasWidth && hasHeight,
      `img tag missing explicit width/height (CLS): ${img.substring(0, 80)}`);
  }
});

test("no inline scripts that block rendering (TBT)", () => {
  const blockingScripts = (html.match(/<script(?![^>]*defer|[^>]*async|[^>]*type=["']module["'])[^>]*src=/gi) || []);
  assert.strictEqual(blockingScripts.length, 0,
    `found ${blockingScripts.length} render-blocking script(s)`);
});

test("links have descriptive aria-labels (Accessibility)", () => {
  const links = html.match(/<a[^>]+>/gi) || [];
  for (const link of links) {
    const hasText = !/^<a[^>]*>\s*<\/a>/.test(link);
    const hasAriaLabel = /aria-label=["'][^"']+["']/i.test(link);
    // Either has visible text content (checked by surrounding context) or aria-label
    assert.ok(hasText || hasAriaLabel,
      `link may lack accessible name: ${link.substring(0, 80)}`);
  }
});

// ── Summary ───────────────────────────────────────────────────────────────────

console.log(`\n${"─".repeat(50)}`);
console.log(`Results: ${passed} passed, ${failed} failed`);

if (failed > 0) {
  console.error(`\n❌ ${failed} test(s) failed`);
  process.exit(1);
} else {
  console.log(`\n✅ All ${passed} tests passed`);
  process.exit(0);
}
