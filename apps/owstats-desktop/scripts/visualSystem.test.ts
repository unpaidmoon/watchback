import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

function withoutComments(css: string) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const tokens = withoutComments(readFileSync(new URL("../src/styles/tokens.css", import.meta.url), "utf8"));
const base = withoutComments(readFileSync(new URL("../src/styles/base.css", import.meta.url), "utf8"));
const shell = withoutComments(readFileSync(new URL("../src/styles/shell.css", import.meta.url), "utf8"));
const entry = withoutComments(readFileSync(new URL("../src/styles.css", import.meta.url), "utf8"));

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

const systemFontStack = '"Microsoft YaHei UI", "PingFang SC", "Noto Sans CJK SC", "Segoe UI", sans-serif';
assert.match(base, new RegExp(`font-family:\\s*${escapeRegExp(systemFontStack)}\\s*;`), "base uses the approved system font stack");

const imports = [...entry.matchAll(/@import\s+"([^"]+)"\s*;/g)].map((match) => match[1]);
assert.deepEqual(
  imports.slice(0, 4),
  ["./styles/tokens.css", "./styles/shared.css", "./styles/base.css", "./styles/shell.css"],
  "visual layers load tokens, legacy declarations, base overrides, then shell overrides",
);
assert.ok(imports.slice(4).every((path) => path.startsWith("./styles/features/")), "feature styles load after the shell layer");

assert.doesNotMatch(shell, /#ff8a24|#d45f14|blue.*orange|orange.*blue/i, "new shell contains no orange brand or blue-orange gradient");
