# Watchback Brand Icon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce an approved Watchback mascot logo and a Windows-ready application icon based on the anthropomorphic battle-journal design.

**Architecture:** Treat the full logo and the application icon as two related assets, not one image at different scales. Use image generation for visual exploration and character refinement, require user approval at each visual checkpoint, then create a clean transparent master, a simplified icon master, and verified Windows-size exports. This plan produces brand assets only; application renaming and product integration remain a separate change.

**Tech Stack:** OpenAI image generation, SVG, PNG, ICO, Tauri CLI icon generator, TypeScript resource tests

---

## File Structure

- `docs/brand/watchback/concepts/`: selected concept sheets retained as design history.
- `docs/brand/watchback/watchback-logo-master.png`: approved transparent full-logo raster master.
- `docs/brand/watchback/watchback-logo-master.svg`: cleaned editable vector master.
- `docs/brand/watchback/watchback-logo-with-descriptor.svg`: standard lockup with the Chinese descriptor `守望战绩工具`.
- `docs/brand/watchback/watchback-logo-with-descriptor.png`: transparent raster export of the standard lockup.
- `docs/brand/watchback/watchback-logo-light.png`: full logo preview for light surfaces.
- `docs/brand/watchback/watchback-logo-dark.png`: full logo preview for dark surfaces.
- `apps/owstats-desktop/src-tauri/icons/watchback-icon-source.png`: approved square 1024px application-icon source.
- `apps/owstats-desktop/src-tauri/icons/`: Tauri-generated platform icon files.
- `apps/owstats-desktop/scripts/brandAssets.test.ts`: deterministic checks for required files, dimensions, and prohibited machine paths.
- `apps/owstats-desktop/package.json`: adds the brand asset test to `test:resources`.

### Task 1: Generate the Character Direction Sheet

**Files:**
- Create: `docs/brand/watchback/concepts/watchback-character-directions.png`

- [ ] **Step 1: Read the image-generation instructions**

Read the complete `imagegen` skill before making any image-generation call. Use the supplied reference image only for the general method of “character + memorable action + hand-drawn wordmark”; do not copy its bear, food, pose, colors, lettering, or composition.

- [ ] **Step 2: Generate one three-panel character direction sheet**

Use the image-generation tool with the approved design reference and this prompt:

```text
Create a clean three-panel logo concept sheet on a neutral warm-white background for a desktop application named “Watchback”.

Show three distinct interpretations of the same original mascot: an anthropomorphic battle journal / match record book. The journal has a charcoal-gray soft cover, warm-white page edges, tiny focused eyes, short arms and legs, and carries a recording pen. It is running toward the right while deliberately turning its gaze back toward a trail made from short lines, dots, and restrained data marks representing past matches. Its personality is attentive, dependable, and quietly warm—not frantic, childish, triumphant, or aggressive.

Style: original flat vector-like hand-drawn logo art, large simple color shapes, slightly irregular human-made contours, clear silhouette, minimal detail, no texture, no 3D rendering, no photorealism. Use only charcoal #303030, warm white #F4F6F0, muted teal #86B8B4, tiny optional purple data points #4B22D1, and gray-green background #E3E8E2.

Each panel must explore a meaningfully different character proportion and running pose while preserving the same action. Do not include Overwatch logos, OW letters, heroes, weapons, armor, game UI, flames, neon colors, yellow explosion shapes, or copyrighted character designs. Do not include any text in this direction sheet.
```

- [ ] **Step 3: Inspect the generated sheet at original resolution**

Check all three panels for:

- a readable journal silhouette;
- a forward-running body with an unmistakable backward gaze;
- a clearly held pen;
- a restrained, reliable expression;
- no accidental `OW` monogram;
- no official game imagery;
- no extra limbs, broken page geometry, illegible marks, or unapproved colors.

- [ ] **Step 4: Present the sheet for user selection**

Show the image and ask the user to choose one panel, reject all panels, or combine explicitly named traits from no more than two panels. Do not proceed to Task 2 without a recorded choice.

### Task 2: Refine the Approved Mascot

**Files:**
- Create: `docs/brand/watchback/concepts/watchback-mascot-refinement-v1.png`
- Create if needed: `docs/brand/watchback/concepts/watchback-mascot-refinement-v2.png`

- [ ] **Step 1: Generate a focused refinement sheet**

Use the selected panel as the image reference. Preserve the chosen body proportion, cover shape, facial placement, and running direction. Generate three close variations that change only:

- backward head/gaze clarity;
- page flutter amount;
- pen angle and arm silhouette;
- match-trail rhythm.

Require the approved five-color palette and a plain warm-white background. Do not introduce text, new props, clothing, weapons, official game elements, gradients, shadows, or textures.

- [ ] **Step 2: Inspect visual consistency**

Compare the three variations against the approved panel. Reject any variation that changes the mascot species/object, personality, running direction, core palette, or book-cover proportion.

- [ ] **Step 3: Present the refinement sheet for approval**

Ask the user to approve one variation. If the user requests a bounded correction, generate `watchback-mascot-refinement-v2.png` with only that correction. Stop after two refinement sheets and discuss the design premise again if no option is acceptable.

### Task 3: Produce the Full Watchback Logo

**Files:**
- Create: `docs/brand/watchback/watchback-logo-master.png`
- Create: `docs/brand/watchback/watchback-logo-light.png`
- Create: `docs/brand/watchback/watchback-logo-dark.png`

- [ ] **Step 1: Generate the transparent full-logo master**

Use the approved mascot as the image reference and request:

```text
Create the final horizontal Watchback brand logo on a transparent background.

Preserve the approved anthropomorphic battle-journal mascot exactly: it runs right while looking back at a restrained trail of past-match data marks. Keep the mascot centered slightly above the wordmark and leave comfortable clear space around the full mark.

Add the English word “Watchback” below the illustration in a legible, slightly irregular hand-drawn black wordmark. The lettering should feel warm and human but mature, stable, and suitable for a desktop data product. Do not imitate the typography of the supplied reference image or any game logo. Do not add the Chinese descriptor to the generated artwork.

Use only #303030, #F4F6F0, #86B8B4, and a very small amount of #4B22D1. Flat colors, no gradients, texture, shadow, glow, mockup, border, or background.
```

- [ ] **Step 2: Inspect text and transparency**

At original resolution, verify:

- `Watchback` is spelled exactly once and every letter is legible;
- no pseudo-text or stray glyphs appear;
- the background is transparent;
- the mascot matches the approved refinement;
- the logo has sufficient clear space and no clipped trail.

If generated lettering is malformed, retain the approved mascot art and replace the wordmark during vector cleanup rather than repeatedly regenerating the mascot.

- [ ] **Step 3: Create light and dark previews**

Place the same transparent master on:

- light preview background `#E3E8E2`;
- dark preview background `#303030`.

The preview images are review artifacts, not alternate logo masters.

- [ ] **Step 4: Obtain final user approval**

Show the transparent logo plus both background previews together. Require explicit approval before generating the application icon.

### Task 4: Produce the Simplified Application Icon

**Files:**
- Create: `apps/owstats-desktop/src-tauri/icons/watchback-icon-source.png`
- Modify: `apps/owstats-desktop/src-tauri/icons/icon.ico`
- Create: `apps/owstats-desktop/src-tauri/icons/32x32.png`
- Create: `apps/owstats-desktop/src-tauri/icons/128x128.png`
- Create: `apps/owstats-desktop/src-tauri/icons/128x128@2x.png`
- Create: `apps/owstats-desktop/src-tauri/icons/icon.icns`

- [ ] **Step 1: Generate a square transparent icon master**

Use the approved mascot reference and request a 1024×1024 icon source with:

- the mascot occupying approximately 70% of the canvas;
- a three-quarter close view of the journal cover;
- focused eyes looking backward;
- one visible pen;
- one short muted-teal curved trail;
- no wordmark, Chinese text, long trail, full running scene, tiny data labels, border, shadow, or background.

Keep generous transparent padding so Windows icon masks do not clip the character.

- [ ] **Step 2: Inspect the 1024px source**

Verify square dimensions, transparency, approved palette, intact anatomy, clean book edges, and recognizable backward gaze.

- [ ] **Step 3: Generate Tauri icon variants**

Run:

```powershell
cd apps\owstats-desktop
npx tauri icon src-tauri/icons/watchback-icon-source.png
```

Expected: Tauri writes the standard PNG, ICO, and ICNS variants under `src-tauri/icons/` without reporting an invalid source image.

- [ ] **Step 4: Inspect the Windows sizes**

Open the generated `icon.ico` and inspect its 16, 24, 32, 48, 64, 128, and 256px frames. At 16px, require only the dark journal body, backward-facing eyes, and muted-teal trail to remain distinct. If the pen or page flutter becomes noise, remove those details from the icon source and regenerate all variants.

- [ ] **Step 5: Present the icon-size comparison for approval**

Show the icon at 16, 24, 32, 48, 64, 128, and 256px on both `#E3E8E2` and `#303030` backgrounds. Require explicit approval before treating the icon as final.

### Task 5: Create the Editable Vector Master

**Files:**
- Create: `docs/brand/watchback/watchback-logo-master.svg`
- Create: `docs/brand/watchback/watchback-logo-with-descriptor.svg`
- Create: `docs/brand/watchback/watchback-logo-with-descriptor.png`

- [ ] **Step 1: Rebuild the approved logo as clean vector shapes**

Trace the approved silhouette manually into grouped SVG paths with these stable group IDs:

```xml
<g id="match-trail">...</g>
<g id="journal-pages">...</g>
<g id="journal-cover">...</g>
<g id="mascot-face">...</g>
<g id="mascot-limbs">...</g>
<g id="recording-pen">...</g>
<g id="watchback-wordmark">...</g>
```

Use explicit hexadecimal fills from the approved palette. Do not embed the raster image, external fonts, linked files, scripts, metadata containing local paths, or base64 payloads.

- [ ] **Step 2: Create the standard Chinese descriptor lockup**

Duplicate the approved logo composition into `watchback-logo-with-descriptor.svg` and add one centered line beneath the English wordmark:

```xml
<text id="chinese-descriptor"
      text-anchor="middle"
      fill="#60645F"
      font-family="Microsoft YaHei UI, PingFang SC, Noto Sans CJK SC, Segoe UI, sans-serif"
      font-size="14"
      font-weight="400"
      letter-spacing="1">守望战绩工具</text>
```

Export the same composition to `watchback-logo-with-descriptor.png` on a transparent background. The English-only master remains the source for the application icon and compact placements.

- [ ] **Step 3: Validate the SVG structure**

Run:

```powershell
rg -n "image href|data:image|script|C:\\|D:\\|/@fs/|url\\(" docs/brand/watchback/watchback-logo-master.svg
rg -n "守望战绩工具" docs/brand/watchback/watchback-logo-with-descriptor.svg
```

Expected: the prohibited-content scan returns no matches, and the descriptor scan returns exactly the `chinese-descriptor` text line.

- [ ] **Step 4: Compare vector and approved raster**

Render both SVG files on transparent, light `#E3E8E2`, and dark `#303030` backgrounds. Confirm that pose, expression, proportions, palette, spacing, English wordmark spelling, and the Chinese descriptor match the approved design.

### Task 6: Add Deterministic Brand Asset Checks

**Files:**
- Create: `apps/owstats-desktop/scripts/brandAssets.test.ts`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Write the failing asset test**

Create `apps/owstats-desktop/scripts/brandAssets.test.ts`:

```ts
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const desktopRoot = resolve(import.meta.dirname, "..");
const repoRoot = resolve(desktopRoot, "../..");

const requiredFiles = [
  resolve(repoRoot, "docs/brand/watchback/watchback-logo-master.png"),
  resolve(repoRoot, "docs/brand/watchback/watchback-logo-master.svg"),
  resolve(repoRoot, "docs/brand/watchback/watchback-logo-with-descriptor.svg"),
  resolve(repoRoot, "docs/brand/watchback/watchback-logo-with-descriptor.png"),
  resolve(desktopRoot, "src-tauri/icons/watchback-icon-source.png"),
  resolve(desktopRoot, "src-tauri/icons/icon.ico"),
];

for (const file of requiredFiles) {
  assert.equal(existsSync(file), true, `missing Watchback brand asset: ${file}`);
}

const svg = readFileSync(requiredFiles[1], "utf8");
assert.match(svg, /id="match-trail"/);
assert.match(svg, /id="journal-cover"/);
assert.match(svg, /id="watchback-wordmark"/);
assert.doesNotMatch(svg, /(?:data:image|<script|C:\\|D:\\|\/@fs\/|https?:\/\/)/i);

const descriptorSvg = readFileSync(requiredFiles[2], "utf8");
assert.match(descriptorSvg, /id="chinese-descriptor"[\s\S]*守望战绩工具/);
assert.doesNotMatch(descriptorSvg, /(?:data:image|<script|C:\\|D:\\|\/@fs\/|https?:\/\/)/i);

const sourcePng = readFileSync(requiredFiles[4]);
assert.equal(sourcePng.subarray(1, 4).toString("ascii"), "PNG");
assert.equal(sourcePng.readUInt32BE(16), 1024);
assert.equal(sourcePng.readUInt32BE(20), 1024);

console.log("Watchback brand asset checks passed");
```

- [ ] **Step 2: Run the test to verify it fails before all assets exist**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/brandAssets.test.ts
```

Expected before Tasks 3–5 are complete: failure naming the first missing brand asset.

- [ ] **Step 3: Add the test to resource verification**

In `apps/owstats-desktop/package.json`, append the command to `test:resources`:

```json
"test:resources": "tsx scripts/runtimeConfig.test.ts && tsx scripts/prepareAssets.test.ts && tsx scripts/verifyDistPaths.test.ts && tsx scripts/phase6Assets.test.ts && tsx scripts/verifyResources.test.ts && tsx scripts/brandAssets.test.ts"
```

- [ ] **Step 4: Run brand and resource tests**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/brandAssets.test.ts
npm run test:resources
```

Expected: both commands exit 0 and print `Watchback brand asset checks passed`.

### Task 7: Final Verification

**Files:**
- Verify only; no new files.

- [ ] **Step 1: Run frontend verification**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: both commands exit 0.

- [ ] **Step 2: Run repository checks**

```powershell
cd D:\Codex\OW_data\owstats-local
git diff --check
git status --short
```

Expected: `git diff --check` exits 0. Review `git status --short` and confirm the Watchback files are the only changes made by this plan; preserve all pre-existing user changes.

- [ ] **Step 3: Report privacy and release scope**

Report that no real credentials, browser profiles, local SQLite databases, private player dumps, or real user data were read or copied. State explicitly that Tauri `productName`, window title, application identifier, frontend brand text, and installer metadata were not changed by this asset-only plan.

- [ ] **Step 4: Commit only if explicitly authorized**

The project instructions prohibit staging or committing without explicit user authorization. If authorization is provided, stage only the Watchback plan files and brand assets, then commit with:

```powershell
git add docs/brand/watchback apps/owstats-desktop/src-tauri/icons apps/owstats-desktop/scripts/brandAssets.test.ts apps/owstats-desktop/package.json
git commit -m "feat: add Watchback brand assets"
```
