import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

const tokens = read("../src/styles/tokens.css");
const legacyShared = read("../src/styles/shared.css");
const base = read("../src/styles/base.css");
const shell = read("../src/styles/shell.css");
const pageShell = read("../src/styles/page-shell.css");
const featureDirectory = new URL("../src/styles/features/", import.meta.url);
const featureStyles = readdirSync(featureDirectory)
  .filter((name) => name.endsWith(".css"))
  .map((name) => read(`../src/styles/features/${name}`))
  .join("\n");
const semanticStyles = [tokens, base, shell, pageShell, featureStyles].join("\n");

assert.match(tokens, /--app-canvas:\s*#E3E8E2/i);
assert.match(tokens, /--action-primary:\s*#303030/i);
assert.match(tokens, /--data-primary:\s*#4B22D1/i);
assert.match(tokens, /--data-secondary:\s*#2F8C95/i);
assert.match(tokens, /--motion-fast:\s*180ms/i);
assert.match(tokens, /--ease-standard:\s*cubic-bezier\([^)]+\)/i, "视觉系统应提供统一缓动令牌");
assert.match(
  base,
  /font-family:\s*"Microsoft YaHei UI",\s*"PingFang SC",\s*"Noto Sans CJK SC",\s*"Segoe UI",\s*sans-serif;/,
);
assert.match(
  base,
  /--phase6-accent:\s*var\(--data-primary\)/,
  "遗留组件的主强调色必须回落到紫色数据令牌",
);
assert.match(
  base,
  /--phase6-blue:\s*var\(--data-secondary\)/,
  "遗留组件的次强调色必须回落到冷蓝绿色数据令牌",
);
assert.match(base, /:where\([^}]+\):focus-visible\s*\{[^}]*outline:\s*2px solid var\(--data-primary\)/s);
assert.match(base, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
assert.match(base, /transition-duration:\s*0\.01ms !important/);
assert.doesNotMatch(semanticStyles, /fonts\.googleapis\.com|https?:\/\/[^)'"\s]+\.(?:woff2?|ttf|otf)/i);
assert.doesNotMatch(semanticStyles, /\/@fs\/|[A-Z]:\\(?:Codex|Users)\\/i);
assert.doesNotMatch(semanticStyles, /linear-gradient\([^)]*(?:orange|#f59|#fb9)[^)]*(?:blue|#2f6|#3b8)/i);
const assetFallbackRule = legacyShared.match(/\.phase6AssetFallback\s*\{([^}]*)\}/s)?.[1] ?? "";
assert.notEqual(assetFallbackRule, "", "共享图片兜底样式必须存在");
assert.doesNotMatch(
  assetFallbackRule,
  /linear-gradient|255\s+138\s+36|47\s+110\s+234/i,
  "共享图片兜底不能回退到蓝橙渐变",
);
assert.match(assetFallbackRule, /background:\s*var\(--surface-secondary\)/, "共享图片兜底应使用中性表面令牌");

const pageState = read("../src/shared/page-shell/PageState.tsx");
assert.match(pageState, /role="status"[^>]*aria-live="polite"[^>]*aria-busy="true"/);
assert.match(pageState, /className="pageState pageState--error" role="alert"/);

// OnboardingPage is intentionally added after the BattleTag integration gate.
for (const page of ["MatchesPage.tsx", "PersonalReviewPage.tsx", "CareerMmrPage.tsx", "RankingPage.tsx"]) {
  assert.match(read(`../src/pages/${page}`), /<PageHeader\b/, `${page} 应使用统一页面标题`);
}
