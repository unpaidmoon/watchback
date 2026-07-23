# Watchback Geometric Brand Icon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the approved B2 “progressive time slices” Watchback mark as deterministic SVG assets, export Windows icon sizes, and verify geometry, colors, portability, and small-size legibility.

**Architecture:** Generate every final vector from one project-local Node script so the standard, monochrome, inverse, small-size, wordmark, and descriptor variants cannot drift. Use a `96 × 96` logical grid, shared geometry constants, and Tauri’s icon generator for platform exports. This asset-only plan does not rename the application or change product metadata.

**Tech Stack:** SVG, Node.js, TypeScript resource tests, Tauri 2 icon generator, `opentype.js`

---

## File Structure

- `apps/owstats-desktop/scripts/buildWatchbackBrand.mjs`: single source of truth for SVG geometry and generated variants.
- `apps/owstats-desktop/scripts/watchbackBrandAssets.test.ts`: deterministic geometry, color, portability, and file-contract checks.
- `apps/owstats-desktop/package.json`: brand build command, development dependency, and resource-test registration.
- `apps/owstats-desktop/assets/brand/watchback/`: generated editable brand SVG files and review sheet.
- `apps/owstats-desktop/src-tauri/icons/watchback-icon-source.svg`: standard square icon source consumed by Tauri.
- `apps/owstats-desktop/src-tauri/icons/`: generated PNG, ICO, and ICNS platform assets.

### Task 1: Add Failing Brand Asset Contracts

**Files:**
- Create: `apps/owstats-desktop/scripts/watchbackBrandAssets.test.ts`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Create the failing asset test**

Create `apps/owstats-desktop/scripts/watchbackBrandAssets.test.ts`:

```ts
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const desktopRoot = resolve(import.meta.dirname, "..");
const brandRoot = resolve(desktopRoot, "assets/brand/watchback");
const iconRoot = resolve(desktopRoot, "src-tauri/icons");

const required = [
  resolve(brandRoot, "watchback-mark.svg"),
  resolve(brandRoot, "watchback-mark-monochrome.svg"),
  resolve(brandRoot, "watchback-mark-inverse.svg"),
  resolve(brandRoot, "watchback-mark-16.svg"),
  resolve(brandRoot, "watchback-mark-24.svg"),
  resolve(brandRoot, "watchback-logo.svg"),
  resolve(brandRoot, "watchback-logo-with-descriptor.svg"),
  resolve(brandRoot, "watchback-review-sheet.svg"),
  resolve(iconRoot, "watchback-icon-source.svg"),
  resolve(iconRoot, "icon.ico"),
];

for (const file of required) {
  assert.equal(existsSync(file), true, `missing Watchback asset: ${file}`);
}

const standard = readFileSync(required[0], "utf8");
assert.match(standard, /viewBox="0 0 96 96"/);
assert.equal((standard.match(/<path\b/g) ?? []).length, 3);
assert.equal((standard.match(/<circle\b/g) ?? []).length, 1);
assert.equal((standard.match(/stroke="#202220"/g) ?? []).length, 3);
assert.match(standard, /fill="#4B22D1"/);
assert.match(standard, /stroke-width="10"/);
assert.match(standard, /stroke-linecap="round"/);

const mono = readFileSync(required[1], "utf8");
assert.doesNotMatch(mono, /#4B22D1/i);
assert.match(mono, /fill="#202220"/);

const inverse = readFileSync(required[2], "utf8");
assert.doesNotMatch(inverse, /#4B22D1/i);
assert.match(inverse, /stroke="#F4F6F0"/);
assert.match(inverse, /fill="#F4F6F0"/);

for (const file of required.slice(0, 9)) {
  const source = readFileSync(file, "utf8");
  assert.doesNotMatch(source, /(?:data:image|<script|(?:href|src)="https?:\/\/|C:\\|D:\\|\/@fs\/)/i);
  assert.doesNotMatch(source, /(?:filter=|linearGradient|radialGradient|<image\b)/i);
}

const descriptor = readFileSync(required[6], "utf8");
assert.match(descriptor, />守望战绩工具<\/text>/);

const small16 = readFileSync(required[3], "utf8");
assert.match(small16, /stroke-width="11\.5"/);

const iconSource = readFileSync(required[8], "utf8");
assert.match(iconSource, /<rect width="96" height="96" rx="22" fill="#F4F6F0"\/>/);

console.log("Watchback brand asset contracts passed");
```

- [ ] **Step 2: Register the test**

Add `tsx scripts/watchbackBrandAssets.test.ts` to the end of `test:resources` in `apps/owstats-desktop/package.json`.

- [ ] **Step 3: Run the test and verify the expected failure**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/watchbackBrandAssets.test.ts
```

Expected: failure naming `watchback-mark.svg` as the first missing asset.

### Task 2: Create the Deterministic SVG Generator

**Files:**
- Create: `apps/owstats-desktop/scripts/buildWatchbackBrand.mjs`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Add the development dependency**

Run:

```powershell
cd apps\owstats-desktop
npm install --save-dev opentype.js
```

Expected: `package.json` and `package-lock.json` record `opentype.js`.

- [ ] **Step 2: Add the brand build command**

Add this script to `apps/owstats-desktop/package.json`:

```json
"brand:build": "node scripts/buildWatchbackBrand.mjs"
```

- [ ] **Step 3: Implement the shared geometry and SVG writer**

Create `apps/owstats-desktop/scripts/buildWatchbackBrand.mjs` with:

```js
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import opentype from "opentype.js";

const desktopRoot = resolve(import.meta.dirname, "..");
const brandRoot = resolve(desktopRoot, "assets/brand/watchback");
const iconRoot = resolve(desktopRoot, "src-tauri/icons");
const fontPath = "C:\\Windows\\Fonts\\segoeui.ttf";

const COLORS = {
  ink: "#202220",
  purple: "#4B22D1",
  warmWhite: "#F4F6F0",
  canvas: "#E3E8E2",
  secondary: "#60645F",
};

const STANDARD_PATHS = [
  "M77 25C64 13 43 14 29 27C15 40 14 61 27 75C38 87 57 89 71 79",
  "M69 39C60 31 47 32 39 40C31 48 31 61 39 69C47 77 59 77 67 70",
  "M61 53C57 49 51 49 47 53C43 57 44 63 48 66",
];

const SMALL_PATHS = [
  "M77 25C64 13 43 14 29 27C15 40 14 61 27 75C38 87 57 89 71 79",
  "M69 39C60 31 47 32 39 40C31 48 31 61 39 69C47 77 59 77 67 70",
  "M60 53C56 50 51 50 48 54C45 58 46 63 50 66",
];

function mark({ ink, node, strokeWidth = 10, paths = STANDARD_PATHS }) {
  return [
    ...paths.map((d) => `<path d="${d}" fill="none" stroke="${ink}" stroke-width="${strokeWidth}" stroke-linecap="round"/>`),
    `<circle cx="77" cy="25" r="${strokeWidth / 2}" fill="${node}"/>`,
  ].join("");
}

function svg({ body, viewBox = "0 0 96 96", width = 96, height = 96, label = "Watchback" }) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="${viewBox}" role="img" aria-label="${label}">${body}</svg>\n`;
}

function pathData(font, text, x, y, size) {
  return font.getPath(text, x, y, size).toPathData(2);
}

async function main() {
  await mkdir(brandRoot, { recursive: true });
  await mkdir(iconRoot, { recursive: true });
  const font = await new Promise((resolveFont, reject) => {
    opentype.load(fontPath, (error, loaded) => error ? reject(error) : resolveFont(loaded));
  });

  const standard = mark({ ink: COLORS.ink, node: COLORS.purple });
  const mono = mark({ ink: COLORS.ink, node: COLORS.ink });
  const inverse = mark({ ink: COLORS.warmWhite, node: COLORS.warmWhite });
  const small16 = mark({ ink: COLORS.ink, node: COLORS.purple, strokeWidth: 11.5, paths: SMALL_PATHS });
  const small24 = mark({ ink: COLORS.ink, node: COLORS.purple, strokeWidth: 11, paths: SMALL_PATHS });

  const wordmark = pathData(font, "Watchback", 118, 64, 34);
  const logo = `<g transform="translate(8 8)">${standard}</g><path id="watchback-wordmark" d="${wordmark}" fill="${COLORS.ink}"/>`;
  const descriptor = `<text x="118" y="88" fill="${COLORS.secondary}" font-family="Microsoft YaHei UI, PingFang SC, Noto Sans CJK SC, Segoe UI, sans-serif" font-size="13" font-weight="400" letter-spacing="1">守望战绩工具</text>`;

  const files = new Map([
    [resolve(brandRoot, "watchback-mark.svg"), svg({ body: standard })],
    [resolve(brandRoot, "watchback-mark-monochrome.svg"), svg({ body: mono })],
    [resolve(brandRoot, "watchback-mark-inverse.svg"), svg({ body: inverse })],
    [resolve(brandRoot, "watchback-mark-16.svg"), svg({ body: small16 })],
    [resolve(brandRoot, "watchback-mark-24.svg"), svg({ body: small24 })],
    [resolve(brandRoot, "watchback-logo.svg"), svg({ body: logo, viewBox: "0 0 320 112", width: 320, height: 112 })],
    [resolve(brandRoot, "watchback-logo-with-descriptor.svg"), svg({ body: `${logo}${descriptor}`, viewBox: "0 0 320 112", width: 320, height: 112, label: "Watchback 守望战绩工具" })],
    [
      resolve(iconRoot, "watchback-icon-source.svg"),
      svg({
        body: `<rect width="96" height="96" rx="22" fill="${COLORS.warmWhite}"/>${standard}`,
        width: 1024,
        height: 1024,
      }),
    ],
  ]);

  for (const [file, contents] of files) await writeFile(file, contents, "utf8");
}

await main();
```

- [ ] **Step 4: Run the generator**

Run:

```powershell
cd apps\owstats-desktop
npm run brand:build
```

Expected: seven files under `assets/brand/watchback/` and `watchback-icon-source.svg` under `src-tauri/icons/`.

### Task 3: Add the Review Sheet and Verify the Vector Assets

**Files:**
- Modify: `apps/owstats-desktop/scripts/buildWatchbackBrand.mjs`
- Create by generator: `apps/owstats-desktop/assets/brand/watchback/watchback-review-sheet.svg`

- [ ] **Step 1: Add a deterministic review-sheet composition**

Add these helpers above `main()`:

```js
function placed(contents, x, y, size) {
  const scale = size / 96;
  return `<g transform="translate(${x} ${y}) scale(${scale})">${contents}</g>`;
}

function label(text, x, y, fill = COLORS.secondary) {
  return `<text x="${x}" y="${y}" fill="${fill}" font-family="Segoe UI, sans-serif" font-size="12" font-weight="400">${text}</text>`;
}
```

Inside `main()`, after creating `logo` and `descriptor`, add:

```js
const smallSizes = [16, 24, 32, 48, 64, 128];
let smallX = 32;
const smallPreview = smallSizes.map((size) => {
  const item = `${placed(size <= 24 ? (size === 16 ? small16 : small24) : standard, smallX, 214 - size, size)}${label(`${size}px`, smallX, 232)}`;
  smallX += Math.max(size, 38) + 18;
  return item;
}).join("");

const reviewSheet = [
  `<rect width="640" height="400" fill="#FFFFFF"/>`,
  `<rect x="20" y="20" width="180" height="170" rx="20" fill="${COLORS.warmWhite}"/>`,
  placed(standard, 46, 40, 128),
  label("standard C2", 64, 176),
  `<rect x="230" y="20" width="180" height="170" rx="20" fill="#FFFFFF" stroke="#D7DAD5"/>`,
  placed(mono, 256, 40, 128),
  label("monochrome", 276, 176),
  `<rect x="440" y="20" width="180" height="170" rx="20" fill="${COLORS.ink}"/>`,
  placed(inverse, 466, 40, 128),
  label("inverse", 504, 176, COLORS.warmWhite),
  smallPreview,
  `<g transform="translate(20 258) scale(0.72)">${logo}</g>`,
  `<g transform="translate(20 324) scale(0.72)">${logo}${descriptor}</g>`,
].join("");
```

Add this entry to `files`:

```js
[resolve(brandRoot, "watchback-review-sheet.svg"), svg({
  body: reviewSheet,
  viewBox: "0 0 640 400",
  width: 640,
  height: 400,
  label: "Watchback brand review sheet",
})],
```

- [ ] **Step 2: Regenerate all vectors**

Run:

```powershell
cd apps\owstats-desktop
npm run brand:build
```

Expected: `watchback-review-sheet.svg` is created or replaced deterministically.

- [ ] **Step 3: Run the asset contracts**

Run:

```powershell
npx tsx scripts/watchbackBrandAssets.test.ts
```

Expected: `Watchback brand asset contracts passed`.

- [ ] **Step 4: Present the review sheet**

Open the review sheet at original resolution and ask the user to inspect:

- whether the three time slices remain distinct at `16px`;
- whether the purple node remains separate from the outer slice;
- whether the mark is optically centered;
- whether the English wordmark feels neutral and professional;
- whether the Chinese descriptor remains subordinate.

Do not generate platform icons until the user approves this sheet.

### Task 4: Generate and Inspect Platform Icons

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/icons/icon.ico`
- Create or modify: Tauri-generated files under `apps/owstats-desktop/src-tauri/icons/`

- [ ] **Step 1: Generate the platform icon set**

Run:

```powershell
cd apps\owstats-desktop
npx tauri icon src-tauri/icons/watchback-icon-source.svg
```

Expected: Tauri exits 0 and produces standard PNG, ICO, and ICNS files under `src-tauri/icons/`.

- [ ] **Step 2: Verify resource presence**

Run:

```powershell
Get-Item src-tauri/icons/icon.ico,src-tauri/icons/32x32.png,src-tauri/icons/128x128.png,src-tauri/icons/128x128@2x.png,src-tauri/icons/icon.icns
```

Expected: every file exists and has non-zero size.

- [ ] **Step 3: Inspect Windows small sizes**

Render or extract `16`, `24`, `32`, `48`, `64`, `128`, and `256px` previews on warm-white and deep-charcoal backgrounds. If any adjacent slices merge, adjust only `SMALL_PATHS`, stroke width, or node radius in the generator, then regenerate and rerun Task 3 tests.

### Task 5: Final Verification

**Files:**
- Verify only.

- [ ] **Step 1: Run frontend tests and build**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: both commands exit 0.

- [ ] **Step 2: Check diffs and scope**

```powershell
cd D:\Codex\OW_data\owstats-local
git diff --check
git status --short
```

Expected: `git diff --check` exits 0. Confirm that this implementation changed only the approved design/plan, brand generator, brand tests, package metadata, generated brand SVG files, and Tauri icon assets. Preserve unrelated user changes.

- [ ] **Step 3: Report privacy and integration boundaries**

Report that no real credentials, browser profiles, local SQLite databases, private player dumps, or real user data were read or copied. State explicitly that Tauri `productName`, application identifier, window title, frontend brand text, and installer metadata were not changed.

- [ ] **Step 4: Commit only with explicit authorization**

If the user explicitly authorizes a commit:

```powershell
git add docs/superpowers/specs/2026-07-24-watchback-brand-icon-design.md docs/superpowers/plans/2026-07-24-watchback-brand-icon.md apps/owstats-desktop/scripts/buildWatchbackBrand.mjs apps/owstats-desktop/scripts/watchbackBrandAssets.test.ts apps/owstats-desktop/package.json apps/owstats-desktop/package-lock.json apps/owstats-desktop/assets/brand/watchback apps/owstats-desktop/src-tauri/icons
git commit -m "feat: add Watchback brand icon"
```
