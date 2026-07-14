import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

function withoutComments(css: string) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function splitSelectors(selectorList: string) {
  const selectors: string[] = [];
  let start = 0;
  let parenthesisDepth = 0;

  for (let index = 0; index < selectorList.length; index += 1) {
    if (selectorList[index] === "(") parenthesisDepth += 1;
    if (selectorList[index] === ")") parenthesisDepth -= 1;
    if (selectorList[index] === "," && parenthesisDepth === 0) {
      selectors.push(selectorList.slice(start, index).trim());
      start = index + 1;
    }
  }

  selectors.push(selectorList.slice(start).trim());
  return selectors;
}

function declarationsForSelector(css: string, selector: string) {
  for (const match of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selectors = splitSelectors(match[1]);
    if (selectors.includes(selector)) return match[2];
  }
  assert.fail(`${selector} is present`);
}

function declarationsForSelectorWithProperty(css: string, selector: string, property: string) {
  const propertyPattern = new RegExp(`(?:^|;)\\s*${escapeRegExp(property)}\\s*:`, "i");
  for (const match of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selectors = splitSelectors(match[1]);
    if (selectors.includes(selector) && propertyPattern.test(match[2])) return match[2];
  }
  assert.fail(`${selector} defines ${property}`);
}

function assertDeclaration(declarations: string, property: string, value: string, message: string) {
  assert.match(
    declarations,
    new RegExp(`(?:^|;)\\s*${escapeRegExp(property)}\\s*:\\s*${escapeRegExp(value)}\\s*;`, "i"),
    message,
  );
}

function blockForAtRule(css: string, atRule: string) {
  const atRuleStart = css.indexOf(atRule);
  assert.notEqual(atRuleStart, -1, `${atRule} is present`);

  const blockStart = css.indexOf("{", atRuleStart);
  assert.notEqual(blockStart, -1, `${atRule} has a block`);

  let depth = 1;
  for (let index = blockStart + 1; index < css.length; index += 1) {
    if (css[index] === "{") depth += 1;
    if (css[index] === "}") depth -= 1;
    if (depth === 0) return css.slice(blockStart + 1, index);
  }

  assert.fail(`${atRule} has a closing brace`);
}

function transitionValues(css: string) {
  const values: string[] = [];
  for (const match of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    for (const declaration of match[2].matchAll(/(?:^|;)\s*transition\s*:\s*([^;]+)\s*;/gi)) {
      values.push(declaration[1].trim());
    }
  }
  return values;
}

const tokens = withoutComments(readFileSync(new URL("../src/styles/tokens.css", import.meta.url), "utf8"));
const base = withoutComments(readFileSync(new URL("../src/styles/base.css", import.meta.url), "utf8"));
const shell = withoutComments(readFileSync(new URL("../src/styles/shell.css", import.meta.url), "utf8"));
const pageShell = withoutComments(readFileSync(new URL("../src/styles/page-shell.css", import.meta.url), "utf8"));
const matchesFeature = withoutComments(readFileSync(new URL("../src/styles/features/matches.css", import.meta.url), "utf8"));
const shared = withoutComments(readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8"));
const entry = withoutComments(readFileSync(new URL("../src/styles.css", import.meta.url), "utf8"));
const app = withoutComments(readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8"));

const approvedTokens = {
  "--app-outside": "#F5F5F3",
  "--app-canvas": "#E3E8E2",
  "--surface-primary": "#F0F1EA",
  "--surface-secondary": "#F6F7F2",
  "--surface-control": "#ECEEE9",
  "--text-primary": "#202220",
  "--text-secondary": "#60645F",
  "--text-tertiary": "#8A8F89",
  "--border-subtle": "#D8DED8",
  "--action-primary": "#303030",
  "--action-on-primary": "#FFFFFF",
  "--data-primary": "#4B22D1",
  "--data-secondary": "#2F8C95",
  "--canvas-glow-cool": "rgba(198, 224, 222, 0.52)",
  "--canvas-glow-light": "rgba(250, 249, 241, 0.72)",
  "--glass-primary": "rgba(244, 246, 240, 0.68)",
  "--glass-secondary": "rgba(250, 251, 247, 0.52)",
  "--glass-border": "rgba(255, 255, 255, 0.58)",
  "--glass-blur": "16px",
  "--data-neutral": "#A5AAA4",
  "--status-success": "#21A366",
  "--status-success-bg": "#E2F3E7",
  "--status-danger": "#E54848",
  "--status-danger-bg": "#F8E7E5",
  "--status-warning": "#B58A16",
  "--status-warning-bg": "#F4EFD8",
  "--space-1": "4px",
  "--space-2": "8px",
  "--space-3": "12px",
  "--space-4": "16px",
  "--space-6": "24px",
  "--space-8": "32px",
  "--space-10": "40px",
  "--radius-shell": "30px",
  "--radius-primary": "20px",
  "--radius-secondary": "16px",
  "--radius-control": "12px",
  "--shadow-card": "0 8px 24px rgba(35, 40, 35, 0.035)",
  "--motion-fast": "180ms",
};

for (const [name, value] of Object.entries(approvedTokens)) {
  assert.match(
    tokens,
    new RegExp(`${escapeRegExp(name)}\\s*:\\s*${escapeRegExp(value)}\\s*;`, "i"),
    `${name} matches the approved value`,
  );
}

const declaredVisualTokens = new Set([...tokens.matchAll(/(--[a-z0-9-]+)\s*:/gi)].map((match) => match[1]));
for (const reference of matchesFeature.matchAll(/var\((--[a-z0-9-]+)(\s*,)?/gi)) {
  const hasFallback = Boolean(reference[2]);
  assert.equal(
    declaredVisualTokens.has(reference[1]) || hasFallback,
    true,
    `matches feature references declared token ${reference[1]} or provides a fallback`,
  );
}

const systemFontStack = '"Microsoft YaHei UI", "PingFang SC", "Noto Sans CJK SC", "Segoe UI", sans-serif';
assert.match(base, new RegExp(`font-family:\\s*${escapeRegExp(systemFontStack)}\\s*;`), "base uses the approved system font stack");

const baseControls = declarationsForSelector(base, "button");
assert.equal(baseControls, declarationsForSelector(base, "input"), "buttons and inputs share the base control rule");
assert.equal(baseControls, declarationsForSelector(base, "select"), "buttons and selects share the base control rule");
assertDeclaration(baseControls, "min-height", "44px", "base controls keep a 44px hit target");
assertDeclaration(baseControls, "min-block-size", "44px !important", "base controls override legacy physical heights with a 44px block target");
assertDeclaration(
  declarationsForSelectorWithProperty(base, "button", "min-inline-size"),
  "min-inline-size",
  "44px !important",
  "buttons override legacy icon widths with a 44px inline target",
);

for (const [legacySelector, property, value] of [
  [".filterBar input", "min-height", "34px"],
  [".filterBar select", "min-height", "34px"],
  [".phase6PlayerSearchBox input", "min-height", "30px"],
  [".phase6PlayerSearchClear", "width", "24px"],
  [".phase6PlayerSearchClear", "height", "24px"],
  [".phase6ShellIconButton", "width", "38px"],
  [".phase6ShellIconButton", "height", "38px"],
  [".iconOnlyButton", "width", "36px"],
  [".iconOnlyButton", "height", "36px"],
  [".phase6IconButton", "width", "34px"],
  [".phase6IconButton", "height", "34px"],
] as const) {
  assertDeclaration(declarationsForSelector(shared, legacySelector), property, value, `${legacySelector} keeps the tracked legacy target conflict`);
}

const playerSearchCompatibility = declarationsForSelector(base, ".phase6PlayerSearchBox");
assertDeclaration(playerSearchCompatibility, "min-block-size", "44px", "player search stays 44px before and after its clear button appears");
assertDeclaration(
  playerSearchCompatibility,
  "grid-template-columns",
  "18px minmax(0, 1fr) 44px",
  "player search reserves the full clear-button target width",
);

const baseFocusRing = declarationsForSelector(base, ":where(a, button, input, select, [tabindex]):focus-visible");
assertDeclaration(baseFocusRing, "outline", "2px solid var(--data-primary) !important", "the unified accessibility layer enforces the purple 2px ring");
assertDeclaration(baseFocusRing, "outline-offset", "2px !important", "the unified accessibility layer enforces the 2px focus offset");
assert.doesNotMatch(baseFocusRing, /forced-color-adjust\s*:\s*none/i, "focus colors remain available for forced-colors mapping");

for (const [legacySelector, property, value] of [
  [".phase6PlayerSearchBox input", "outline", "none"],
  [".searchBox input", "outline", "0"],
  [".seasonSelect select", "outline", "0"],
  [".phase6CompactPlayerRow.phase7CompactPlayerExpandable:focus-visible", "outline", "2px solid rgb(255 255 255 / 0.9)"],
  [".phase7ArenaSlot", "outline", "1px solid rgb(255 255 255 / 0.1)"],
  [".phase7ArenaSlot.filled", "outline-color", "rgb(255 255 255 / 0.18)"],
] as const) {
  assertDeclaration(declarationsForSelector(shared, legacySelector), property, value, `${legacySelector} keeps the tracked legacy focus conflict`);
}

const reducedMotion = blockForAtRule(base, "@media (prefers-reduced-motion: reduce)");
const reducedMotionReset = declarationsForSelector(reducedMotion, "*");
assert.equal(reducedMotionReset, declarationsForSelector(reducedMotion, "*::before"), "reduced motion covers before pseudo-elements");
assert.equal(reducedMotionReset, declarationsForSelector(reducedMotion, "*::after"), "reduced motion covers after pseudo-elements");
for (const [property, value] of Object.entries({
  "scroll-behavior": "auto !important",
  "animation-duration": "0.01ms !important",
  "animation-iteration-count": "1 !important",
  "transition-duration": "0.01ms !important",
})) {
  assertDeclaration(reducedMotionReset, property, value, `reduced motion sets ${property} safely`);
}

const imports = [...entry.matchAll(/@import\s+"([^"]+)"\s*;/g)].map((match) => match[1]);
assert.deepEqual(
  imports.slice(0, 5),
  [
    "./styles/tokens.css",
    "./styles/shared.css",
    "./styles/base.css",
    "./styles/shell.css",
    "./styles/page-shell.css",
  ],
  "visual layers load tokens, legacy declarations, base, shell, then page-shell overrides",
);
assert.ok(imports.slice(5).every((path) => path.startsWith("./styles/features/")), "feature styles load after the page-shell layer");

assert.doesNotMatch(shell, /#ff8a24|#d45f14|blue.*orange|orange.*blue/i, "new shell contains no orange brand or blue-orange gradient");
assert.match(shell, /radial-gradient\(/i, "应用画布应有大范围柔和光晕");
assert.match(shell, /@supports\s*\(backdrop-filter:\s*blur\(1px\)\)/i, "毛玻璃必须有特性检测与回退");
assert.doesNotMatch(shell, /animation:\s*(?!none)/i, "指挥台画布不得运行持续动画");

declarationsForSelector(shared, ".phase6ProductNav a");
declarationsForSelector(shared, ".phase6ProductNav a.active");
assert.match(app, /className="appNav phase6ProductNav"/, "App combines the semantic and legacy nav container classes");
assert.match(app, /isActive\s*\?\s*"appNavLink active"\s*:\s*"appNavLink"/, "App combines semantic nav links with the active state");

const compatibleNavLink = declarationsForSelector(shell, ".phase6ProductNav a.appNavLink");
for (const [property, value] of Object.entries({
  "min-height": "44px",
  color: "var(--text-secondary)",
  background: "var(--surface-control)",
  border: "1px solid var(--surface-control)",
  "border-radius": "var(--radius-control)",
  "font-size": "13px",
  "font-weight": "500",
  "box-shadow": "none",
})) {
  assertDeclaration(compatibleNavLink, property, value, `legacy-compatible nav ${property} uses the semantic visual system`);
}

const semanticNavLink = declarationsForSelector(shell, ".appNavLink");
assertDeclaration(semanticNavLink, "min-height", "44px", "semantic navigation keeps a 44px hit target");
assert.doesNotMatch(semanticNavLink, /(?:^|;)\s*transition\s*:/i, "navigation does not declare a transition without an opacity or transform state change");
const dataEntry = declarationsForSelector(shell, ".appDataEntry");
assertDeclaration(dataEntry, "min-height", "44px", "local data entry keeps a 44px hit target");

assertDeclaration(declarationsForSelector(shared, ".phase6ProductNav a"), "min-height", "36px", "the test tracks the legacy nav height override");

const pageStateAction = declarationsForSelector(pageShell, ".pageStateAction");
assertDeclaration(pageStateAction, "min-height", "44px", "page state actions keep a 44px hit target");
assertDeclaration(pageStateAction, "transition", "opacity var(--motion-fast) ease", "page state hover opacity uses the fast motion token");

const regularTransitions = [...transitionValues(shell), ...transitionValues(pageShell)];
assert.ok(regularTransitions.length > 0, "shell layers define intentional interaction transitions");
for (const transition of regularTransitions) {
  for (const item of transition.split(",")) {
    assert.match(
      item.trim(),
      /^(?:opacity|transform)\s+var\(--motion-fast\)(?:\s+[a-z-]+(?:\([^)]*\))?)?$/i,
      `shell transition only animates opacity or transform with --motion-fast: ${item.trim()}`,
    );
  }
}

const compatibleNavHover = declarationsForSelector(shell, ".phase6ProductNav a.appNavLink:hover");
assertDeclaration(compatibleNavHover, "color", "var(--text-primary)", "legacy-compatible hover uses semantic text");
assertDeclaration(compatibleNavHover, "background", "var(--surface-secondary)", "legacy-compatible hover uses semantic background");

const compatibleNavFocus = declarationsForSelector(shell, ".phase6ProductNav a.appNavLink:focus-visible");
assertDeclaration(compatibleNavFocus, "color", "var(--text-primary)", "legacy-compatible focus uses semantic text");
assertDeclaration(compatibleNavFocus, "background", "var(--surface-secondary)", "legacy-compatible focus uses semantic background");

const compatibleNavActive = declarationsForSelector(shell, ".phase6ProductNav a.appNavLink.active");
assert.equal(
  compatibleNavActive,
  declarationsForSelector(shell, '.phase6ProductNav a.appNavLink[aria-current="page"]'),
  "class and aria-current active states share one rule",
);
assertDeclaration(compatibleNavActive, "color", "var(--action-on-primary)", "legacy-compatible active uses action text");
assertDeclaration(compatibleNavActive, "background", "var(--action-primary)", "legacy-compatible active uses action background");
assertDeclaration(compatibleNavActive, "border-color", "var(--action-primary)", "legacy-compatible active uses action border");
assertDeclaration(compatibleNavActive, "box-shadow", "none", "legacy-compatible active removes the old orange shadow");

const activeRuleIndex = shell.indexOf(".phase6ProductNav a.appNavLink.active");
assert.ok(activeRuleIndex > shell.indexOf(".phase6ProductNav a.appNavLink:hover"), "active styling wins while hovered");
assert.ok(activeRuleIndex > shell.indexOf(".phase6ProductNav a.appNavLink:focus-visible"), "active styling wins while focused");
