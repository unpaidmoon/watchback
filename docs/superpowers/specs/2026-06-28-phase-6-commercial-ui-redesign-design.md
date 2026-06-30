# Phase 6 Commercial UI Redesign Design

## Status

This document captures the agreed Phase 6 frontend direction before implementation.
Phase 6 is a major UI redesign for the existing Tauri desktop product. It does
not change the core local data model or the Phase 4/5 product capabilities unless
explicitly called out here.

## Goal

Turn the current function-complete desktop app into a commercial-quality data
product UI.

The target experience is a polished Overwatch analytics workstation: clear
status, confident navigation, rich hero/map visuals, readable dense data, and a
calm glassmorphism surface inspired by Botrix-style AI command center dashboard
design.

Success means the app should no longer feel like an engineering/debug interface.
It should feel like a product that a player can open repeatedly to review
matches, compare heroes/maps, and understand season performance without needing
developer context.

## Collaboration Workflow

Phase 6 will use live-preview collaboration rather than screenshot handoff.

1. Codex starts the local frontend dev server under `apps/owstats-desktop`.
2. The user opens the local URL in the Codex/browser environment.
3. The user gives feedback against the live page using concise annotations:
   - Page
   - Area
   - Problem
   - Expected result
   - Priority when needed
4. Codex applies focused UI changes while keeping the dev server available.
5. The user refreshes and reviews the real interface.
6. Each iteration handles a bounded surface, such as app shell, match list, match
   detail, ranking, or review.

Example feedback format:

```text
Page: Match list
Area: Top filters
Problem: Too many controls compete for attention.
Expected: Show season, hero, and result first. Put advanced filters behind an
expandable control.
Priority: High
```

Screenshots may still be used for final records or bug reports, but they are not
the primary collaboration method.

## Visual Reference

Primary visual reference:

- Botrix AI Command Center Dashboard Design:
  `https://me.muz.li/orbix-studio/botrix-ai-command-center-dashboard-design-2`

The desired qualities from the reference:

- Soft neutral background.
- Frosted glass panels.
- Rounded command-center navigation.
- Large top metrics.
- Subtle charts and micro-visualizations.
- Calm, premium, data-driven tone.
- Strong spacing and visual hierarchy.

The app should be inspired by this reference, not copied exactly.

OWStats has denser tables, Chinese text, match details, and repeated analysis
workflows. The redesign must preserve data readability even when the reference
uses lower-contrast, presentation-oriented UI.

## Visual Direction

Adopt a "light glass analytics workstation" direction.

Core attributes:

- Soft light background with restrained warm/cool gradients.
- Glass panels using translucent backgrounds, blur, fine borders, and soft
  shadows.
- High-readability data surfaces, especially tables and detailed comparison
  blocks.
- Orange as the primary product/action accent.
- Green/red for positive/negative outcome and sync state.
- Blue/purple only as secondary chart accents.
- Lucide icons for navigation, buttons, status, and toolbar actions.
- Rounded but controlled geometry. Cards and panels may use larger radii where
  glass style benefits from it, but dense controls should remain compact and
  stable.

Implementation defaults:

- Use CSS `backdrop-filter` and `-webkit-backdrop-filter` for frosted panels.
- Use more opaque panel backgrounds for tables and long text regions.
- Avoid stacking excessive blur layers in scroll-heavy regions.
- Provide a graceful visual fallback where blur is unavailable.
- Do not use decorative blobs/orbs as primary design content. Background
  gradients should stay subtle and non-distracting.

## Product Information Architecture

The redesigned UI should clarify the app's main product areas:

- Setup / command center: login, data status, sync, and recommended next action.
- Matches: only place for filtering and detailed match browsing.
- Match detail: one match's result, context, score, performance, evidence, and
  same-role comparison.
- Personal review: season-level findings and evidence links.
- Ranking: hero and map performance rankings.
- Friend analysis: teammate boards, friend details, and map-level season
  insights.

Navigation must feel product-owned, not debug-owned. Phase 6 should reevaluate
whether the final app shell uses:

- Top command-center tabs, closer to the Botrix reference.
- A left rail plus top status bar, more suitable for dense desktop workflows.
- A hybrid shell with compact top navigation and persistent global status.

The first implementation plan should include a small live style board or app-shell
prototype so the user can choose the shell direction before all pages are
converted.

## Assets

The user has provided local visual assets at:

```text
assets/
assets/heroes/
assets/maps/
assets/manifest.json
```

Expected use:

- Hero portraits should appear next to hero names where it helps recognition:
  match rows, match detail, hero ranking, hero filters, and review evidence.
- Map preview images should appear in match list rows, match detail, map ranking,
  and map analysis cards.
- Images should be used as product visuals, not decorative filler.
- Missing asset cases must degrade gracefully to a text label or neutral
  placeholder.
- Asset lookup should prefer stable hero/map keys already used by DTOs and
  filters. Do not rely on display text matching as the primary lookup mechanism
  if stable identifiers are available.
- Existing upstream/internal DTO values should remain unchanged. Asset mapping
  belongs in frontend asset helpers or a clearly isolated mapping module.

Implementation notes:

- The current `assets/manifest.json` contains source metadata and local paths.
  During implementation, verify whether manifest paths match the final app asset
  serving path. If not, create a frontend-facing manifest/helper without changing
  backend DTO contracts.
- For Tauri packaging, assets must be available in built app output. The plan
  must include copying or bundling the asset directory through Vite/Tauri in a
  deterministic way.
- Optimize display sizes in CSS. Do not load large map previews at full visual
  size when a compact thumbnail is enough.

## Debug UI Removal

Phase 6 should remove or hide developer/debug-facing surfaces from the product
UI.

Remove from normal product screens:

- Data health cards that read as internal diagnostics rather than user-facing
  status.
- Redacted raw JSON viewers.
- Labels that mention debug, raw payload, DTOs, schemas, sanitized JSON, or
  internal implementation details.
- Placeholder cards that only explain moved/removed technical features.
- Any display path that exposes token-like, cookie-like, customerToken-like, or
  raw player privacy identifiers.

Keep product-safe status where useful:

- Login state.
- Data availability.
- Last sync time.
- Sync progress.
- Recoverable error messages.
- Clear actions such as log in again, sync season, open matches, or view review.

The product can still keep diagnostics in development-only code paths if needed,
but Phase 6 UI should not present them as primary user features.

## Page-Level Redesign Scope

### App Shell

Responsibilities:

- Provide clear primary navigation.
- Show product identity and current global state.
- Surface sync/login state without exposing secrets.
- Avoid crowded navigation as modules grow.

Acceptance:

- The first viewport communicates what the app is for.
- Current section is obvious.
- Navigation labels are Chinese and concise.
- Global actions do not compete with page-specific actions.

### Setup / Command Center

Responsibilities:

- Replace the current setup/debug feel with a product command center.
- Show account state, local data state, season sync state, and recommended next
  action.
- Present the core metrics in a Botrix-like top metric strip.

Candidate metrics:

- Credential state.
- Current season.
- Local match count.
- Detail match count.
- Last sync time.
- Data freshness / ready state.

Acceptance:

- A new user understands how to log in and sync.
- A returning user understands whether data is ready.
- Secret values and raw credential material are never shown.

### Matches

Responsibilities:

- Make the match list feel like a data product, not a raw table.
- Use hero portraits and map previews for fast scanning.
- Keep filtering only in this module.
- Reduce visual weight of advanced filters.

Acceptance:

- Match rows show result, map, hero, mode, score, time, and evaluation clearly.
- Map preview and hero portrait improve recognition without making rows too tall.
- Empty/loading/error states are product-grade Chinese copy.
- Route query filter behavior from Phase 4 remains intact.

### Match Detail

Responsibilities:

- Create a match report page.
- Use map preview as the contextual hero visual.
- Show hero portrait with player performance.
- Preserve same-role comparison and evidence links.

Acceptance:

- The page answers: what happened, how did I perform, and what evidence supports
  the conclusion?
- Image treatment improves context but does not obscure data.
- Same-role comparison remains easy to read.

### Personal Review

Responsibilities:

- Present season review as a concise report.
- Remove debug-looking cards and raw diagnostics.
- Emphasize conclusions, evidence, and next actions.

Acceptance:

- User can identify the main season finding within a few seconds.
- Evidence links are visible and trustworthy.
- No raw JSON or developer wording remains.

### Ranking

Responsibilities:

- Make hero and map rankings visually rich and scannable.
- Add hero portraits to hero ranking.
- Add map previews to map ranking.
- Keep data density and sort clarity.

Acceptance:

- Rankings remain readable at desktop widths.
- Image thumbnails are consistent and do not cause layout shift.
- Search and tab behavior remain unchanged.

### Friend Analysis

Responsibilities:

- Present teammate boards as product insights.
- Keep friend detail access.
- Use supporting visuals where helpful, especially hero/map references.

Acceptance:

- Board purpose is clear from title, metric, and reason text.
- Friend cards do not look like debug panels.
- Modal/detail views remain readable and visually consistent.

## Component System

Phase 6 should introduce or consolidate shared frontend components instead of
restyling every page independently.

Likely shared pieces:

- `AppShell`
- `GlassPanel`
- `MetricCard`
- `StatusBadge`
- `HeroAvatar`
- `MapPreview`
- `PageHeader`
- `Toolbar`
- `SegmentedControl`
- `EmptyState`
- `ErrorBanner`
- `LoadingState`
- `DataTable` or table styling primitives

Design tokens should cover:

- Color palette.
- Semantic status colors.
- Typography scale.
- Spacing scale.
- Border radius.
- Shadow levels.
- Glass background and border recipes.
- Z-index for modals and overlays.

## Accessibility and Readability

Phase 6 must not sacrifice usability for the glass effect.

Requirements:

- Chinese text must not overflow buttons, tabs, cards, or table cells.
- Long values should truncate intentionally with titles/tooltips where needed.
- Interactive controls must have visible focus states.
- Icons used without labels need accessible labels or titles.
- Color cannot be the only way to understand win/loss or positive/negative
  values.
- Table rows and cards must remain readable against translucent backgrounds.

## Performance and Packaging

Risks:

- Backdrop blur can be expensive when overused.
- Map previews and hero portraits increase asset load and package size.
- Dense lists with images can feel sluggish if thumbnails are not sized
  predictably.

Mitigations:

- Use blur mostly on shell, panels, and static containers.
- Avoid heavy blur on every row in long lists.
- Define fixed thumbnail dimensions.
- Lazy-load images where useful.
- Bundle assets deterministically for both dev and Tauri production builds.
- Verify `npm run build` after asset integration.

## Implementation Order

Recommended order:

1. Build a live Phase 6 style board or app-shell prototype with 2-3 shell
   variants based on the Botrix-inspired direction.
2. User chooses the preferred shell and glass intensity.
3. Add design tokens and shared components.
4. Implement asset helper(s) for hero portraits and map previews.
5. Redesign app shell and setup/command center.
6. Redesign match list and match detail.
7. Redesign ranking.
8. Redesign personal review and friend analysis.
9. Remove debug/raw JSON/data-health product UI.
10. Run build verification and live visual review across key desktop window
    sizes.

## Acceptance Criteria

Phase 6 is accepted when:

- The app uses a coherent Botrix-inspired glassmorphism visual system.
- The app still feels like an Overwatch analytics product, not a generic AI
  dashboard clone.
- User-facing frontend text is Chinese.
- Hero portraits appear beside hero names in appropriate high-value locations.
- Map previews appear in match, match detail, map ranking, and map insight
  contexts where useful.
- Missing images degrade gracefully.
- Product screens no longer show normal-user raw JSON/debug/data-health cards.
- Login, sync, match browsing, review, ranking, and friend analysis remain
  reachable.
- Existing filtering and route restoration behavior remains intact.
- Dense tables remain readable despite the glass style.
- `npm run build` passes.
- Final live review confirms no obvious text overlap, broken image, or unreadable
  glass panel in common desktop window sizes.

## Non-Goals

- No backend analytics redesign unless required to support existing UI data.
- No new multi-account feature.
- No online account system.
- No cloud storage.
- No monetization or license enforcement UI.
- No exact copy of the Botrix reference layout.
- No Figma-level annotation system inside the app; live-preview text feedback is
  the agreed collaboration model.

## Design Self-Review

Placeholder check:

- No unresolved placeholder requirements remain. The exact shell variant is
  intentionally deferred to a live style-board choice, which is part of the
  defined Phase 6 process rather than an undefined requirement.

Consistency check:

- The design keeps Phase 4's product boundaries: match filtering stays in the
  match module, ranking owns hero/map rankings, review stays report-focused, and
  data remains local.
- The asset plan does not require backend DTO renaming.
- Debug UI removal preserves product-safe status and error handling.

Scope check:

- This is a large but coherent frontend phase. It should be implemented in
  batches with live review between batches.

Ambiguity check:

- "Glassmorphism" is defined as translucent panel surfaces with blur, border,
  shadow, and readable fallbacks. It is not permission to make all content low
  contrast.
