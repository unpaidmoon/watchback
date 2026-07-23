# Watchback Product Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the remaining user-visible OWStatsLocal branding with Watchback and use the approved geometric mark in the global capsule-navigation brand area without changing compatibility-sensitive internal identifiers.

**Architecture:** Keep the existing Tauri identifier, data paths, credential target, Rust crate, and internal API unchanged. Update only user-visible configuration and shell presentation. Import the tracked SVG through Vite so the brand mark is bundled, and protect the boundary with a source-level integration contract.

**Tech Stack:** React, TypeScript, CSS, Vite asset imports, Tauri 2 JSON configuration, Node/tsx tests

---

## File Structure

- `apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts`: user-visible branding and compatibility-boundary contract.
- `apps/owstats-desktop/package.json`: registers the integration contract.
- `apps/owstats-desktop/src-tauri/tauri.conf.json`: Watchback installer/product and window title.
- `apps/owstats-desktop/index.html`: Watchback WebView title.
- `apps/owstats-desktop/src/App.tsx`: Watchback name, descriptor, and bundled brand mark.
- `apps/owstats-desktop/src/styles/shell.css`: warm-white rounded-square mark container.
- `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`: synthetic visual fixture branding.
- `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`: synthetic fixture product name.

### Task 1: Add a Failing Product Integration Contract

**Files:**
- Create: `apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Create the failing source contract**

Create `apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts`:

```ts
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

function source(relativePath: string) {
  return readFileSync(new URL(`../${relativePath}`, import.meta.url), "utf8");
}

const tauriConfig = JSON.parse(source("src-tauri/tauri.conf.json")) as {
  productName: string;
  identifier: string;
  app: { windows: Array<{ title: string }> };
};
const html = source("index.html");
const app = source("src/App.tsx");
const shell = source("src/styles/shell.css");
const fixture = source("scripts/matchWorkbenchVisualFixture.tsx");
const acceptanceFixture = source("scripts/uiVisualAcceptance.fixture.ts");
const credentialStore = source("src-tauri/src/deepseek_key.rs");

assert.equal(tauriConfig.productName, "Watchback");
assert.equal(tauriConfig.app.windows[0]?.title, "Watchback");
assert.equal(tauriConfig.identifier, "local.owstats.desktop");

assert.match(html, /<title>Watchback<\/title>/);
assert.match(app, /const watchbackMarkUrl = new URL\("\.\.\/assets\/brand\/watchback\/watchback-mark\.svg", import\.meta\.url\)\.href;/);
assert.match(app, /<img src=\{watchbackMarkUrl\} alt="" \/>/);
assert.match(app, /<strong className="appBrandName">Watchback<\/strong>/);
assert.match(app, /<p>守望战绩工具<\/p>/);
assert.doesNotMatch(app, /Sparkles|OWStatsLocal/);

assert.match(shell, /\.appBrand > \.phase6ProductLogo\s*\{[^}]*border-radius:\s*var\(--radius-control\)/s);
assert.match(shell, /\.appBrand > \.phase6ProductLogo\s*\{[^}]*background:\s*#F4F6F0/s);
assert.match(shell, /\.appBrand > \.phase6ProductLogo img\s*\{[^}]*width:\s*32px[^}]*height:\s*32px/s);
assert.doesNotMatch(shell, /\.appBrand > \.phase6ProductLogo\s*\{[^}]*background:\s*var\(--action-primary\)/s);

assert.match(fixture, /<strong className="appBrandName">Watchback<\/strong>/);
assert.doesNotMatch(fixture, /OWStatsLocal/);
assert.match(acceptanceFixture, /productName:\s*"Watchback"/);

assert.match(credentialStore, /OWStatsLocal\/DeepSeekApiKey/);
assert.match(credentialStore, /wide\("OWStatsLocal"\)/);

console.log("Watchback product integration contracts passed");
```

- [ ] **Step 2: Register the contract**

Append `tsx scripts/watchbackProductIntegration.test.ts` to `test:resources` in `apps/owstats-desktop/package.json`.

- [ ] **Step 3: Verify the expected RED failure**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/watchbackProductIntegration.test.ts
```

Expected: failure because `tauriConfig.productName` is still `OWStatsLocal`.

### Task 2: Update User-Visible Application Names

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tauri.conf.json`
- Modify: `apps/owstats-desktop/index.html`

- [ ] **Step 1: Update Tauri display names**

Change only these values in `src-tauri/tauri.conf.json`:

```json
"productName": "Watchback"
```

and:

```json
"title": "Watchback"
```

Keep:

```json
"identifier": "local.owstats.desktop"
```

- [ ] **Step 2: Update the WebView title**

In `index.html`, use:

```html
<title>Watchback</title>
```

- [ ] **Step 3: Re-run the contract**

Run:

```powershell
npx tsx scripts/watchbackProductIntegration.test.ts
```

Expected: the test advances past Tauri and HTML assertions, then fails because `App.tsx` still lacks the Watchback SVG import.

### Task 3: Integrate the Mark into the Global Shell

**Files:**
- Modify: `apps/owstats-desktop/src/App.tsx`
- Modify: `apps/owstats-desktop/src/styles/shell.css`

- [ ] **Step 1: Replace the placeholder icon and text**

In `src/App.tsx`, remove:

```ts
import { Sparkles } from "lucide-react";
```

Add after the imports:

```ts
const watchbackMarkUrl = new URL("../assets/brand/watchback/watchback-mark.svg", import.meta.url).href;
```

Replace the brand block with:

```tsx
<div className="appBrand phase6ProductBrand">
  <span className="phase6ProductLogo" aria-hidden="true">
    <img src={watchbackMarkUrl} alt="" />
  </span>
  <div>
    <strong className="appBrandName">Watchback</strong>
    <p>守望战绩工具</p>
  </div>
</div>
```

- [ ] **Step 2: Apply the approved icon-container styling**

Replace the `.appBrand > .phase6ProductLogo` rule in `src/styles/shell.css` with:

```css
.appBrand > .phase6ProductLogo {
  border: 1px solid var(--glass-border);
  border-radius: var(--radius-control);
  background: #F4F6F0;
  color: var(--text-primary);
  box-shadow: none;
}

.appBrand > .phase6ProductLogo img {
  display: block;
  width: 32px;
  height: 32px;
}
```

Do not change `.appTopBar` height, grid columns, or navigation spacing.

- [ ] **Step 3: Re-run the contract**

Run:

```powershell
npx tsx scripts/watchbackProductIntegration.test.ts
```

Expected: the test advances through the application shell assertions, then fails on the old visual fixture name.

### Task 4: Synchronize Synthetic Visual Fixtures

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`

- [ ] **Step 1: Update the match-workbench fixture brand**

In `matchWorkbenchVisualFixture.tsx`, change:

```tsx
<span className="fixtureBrandMark" aria-hidden="true">OW</span>
```

to:

```tsx
<span className="fixtureBrandMark" aria-hidden="true">WB</span>
```

Change the fixture title to:

```tsx
<strong className="appBrandName">Watchback</strong>
```

Change the fixture descriptor to:

```tsx
<p>守望战绩工具</p>
```

- [ ] **Step 2: Update the visual-acceptance fixture name**

In `uiVisualAcceptance.fixture.ts`, use:

```ts
productName: "Watchback",
```

- [ ] **Step 3: Verify GREEN**

Run:

```powershell
npx tsx scripts/watchbackProductIntegration.test.ts
```

Expected: `Watchback product integration contracts passed`.

### Task 5: Run Focused and Full Verification

**Files:**
- Verify only.

- [ ] **Step 1: Run focused shell and brand contracts**

```powershell
cd apps\owstats-desktop
npx tsx scripts/watchbackProductIntegration.test.ts
npx tsx scripts/watchbackBrandAssets.test.ts
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/visualSystem.test.ts
```

Expected: all commands exit 0.

- [ ] **Step 2: Run frontend tests and build**

```powershell
npm test
npm run build
```

Expected: both commands exit 0.

- [ ] **Step 3: Verify Tauri configuration**

```powershell
npx tauri info
```

Expected: command exits 0 and recognizes the Tauri 2 project configuration.

- [ ] **Step 4: Check scope and formatting**

```powershell
cd D:\Codex\OW_data\owstats-local
git diff --check
git status --short
```

Expected: only the approved integration plan/spec, Tauri config, HTML title, React shell, shell CSS, fixtures, package metadata, and integration contract are newly changed. The rejected `docs/brand/` concept files remain untracked and excluded.

- [ ] **Step 5: Report compatibility and privacy boundaries**

Report that `local.owstats.desktop`, SQLite/data paths, `OWStatsLocal/DeepSeekApiKey`, the Credential Manager username, Rust crate names, internal APIs, and upstream fields remained unchanged. Report that no real credentials, browser profiles, local databases, or real user data were read.

- [ ] **Step 6: Commit only if explicitly authorized**

If the user explicitly authorizes a commit:

```powershell
git add docs/superpowers/specs/2026-07-24-watchback-brand-icon-design.md docs/superpowers/plans/2026-07-24-watchback-product-integration.md apps/owstats-desktop/src-tauri/tauri.conf.json apps/owstats-desktop/index.html apps/owstats-desktop/src/App.tsx apps/owstats-desktop/src/styles/shell.css apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts apps/owstats-desktop/package.json
git commit -m "feat: integrate Watchback branding"
```
