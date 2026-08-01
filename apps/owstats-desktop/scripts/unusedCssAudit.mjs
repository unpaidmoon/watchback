import { readFileSync, readdirSync } from "node:fs";
import { extname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = fileURLToPath(new URL("..", import.meta.url));
const sourceRoot = fileURLToPath(new URL("../src/", import.meta.url));
const stylesRoot = fileURLToPath(new URL("../src/styles/", import.meta.url));

const dynamicClassAllowlist = [
  /^badgeTone-(?:gold|blue|muted|danger)$/,
  /^statusPill--(?:success|danger|warning)$/,
  /^tier-[0-3]$/,
  /^qrAccountLoginQrStage--(?:loading|ready|confirming|failed|expired)$/,
  /^qrAccountLoginStatus--(?:loading|ready|confirming|failed|expired)$/,
  /^result-(?:win|loss|draw|neutral)$/,
];

function walk(directory, extensions) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return walk(path, extensions);
    return extensions.has(extname(entry.name)) ? [path] : [];
  });
}

function stripComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, " "));
}

function splitSelectors(header) {
  const selectors = [];
  let start = 0;
  let parentheses = 0;
  let brackets = 0;
  for (let index = 0; index < header.length; index += 1) {
    const character = header[index];
    if (character === "(") parentheses += 1;
    if (character === ")") parentheses -= 1;
    if (character === "[") brackets += 1;
    if (character === "]") brackets -= 1;
    if (character === "," && parentheses === 0 && brackets === 0) {
      selectors.push(header.slice(start, index).trim());
      start = index + 1;
    }
  }
  selectors.push(header.slice(start).trim());
  return selectors.filter(Boolean);
}

function matchingBrace(css, openingBrace) {
  let depth = 1;
  let quote = null;
  for (let index = openingBrace + 1; index < css.length; index += 1) {
    const character = css[index];
    if (quote) {
      if (character === "\\") index += 1;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === "\"" || character === "'") quote = character;
    else if (character === "{") depth += 1;
    else if (character === "}" && --depth === 0) return index;
  }
  throw new Error(`Unclosed CSS block at offset ${openingBrace}`);
}

function collectRules(css, offset = 0, output = []) {
  let cursor = 0;
  while (cursor < css.length) {
    const openingBrace = css.indexOf("{", cursor);
    if (openingBrace < 0) break;
    const closingBrace = matchingBrace(css, openingBrace);
    const headerStart = Math.max(css.lastIndexOf("}", openingBrace - 1) + 1, cursor);
    const header = css.slice(headerStart, openingBrace).trim();
    const body = css.slice(openingBrace + 1, closingBrace);
    const absoluteHeaderStart = offset + headerStart + css.slice(headerStart, openingBrace).indexOf(header);

    if (/^@(media|supports|layer|container|document)\b/i.test(header)) {
      collectRules(body, offset + openingBrace + 1, output);
    } else if (!header.startsWith("@") && header) {
      output.push({ header, offset: absoluteHeaderStart });
    }
    cursor = closingBrace + 1;
  }
  return output;
}

const sourceText = walk(sourceRoot, new Set([".ts", ".tsx"]))
  .map((file) => readFileSync(file, "utf8"))
  .join("\n");

function sourceMentionsClass(className) {
  const escaped = className.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`(?<![A-Za-z0-9_-])${escaped}(?![A-Za-z0-9_-])`).test(sourceText);
}

function isDynamicClass(className) {
  return dynamicClassAllowlist.some((pattern) => pattern.test(className));
}

let candidates = 0;
for (const file of walk(stylesRoot, new Set([".css"]))) {
  const css = stripComments(readFileSync(file, "utf8"));
  for (const rule of collectRules(css)) {
    const line = css.slice(0, rule.offset).split("\n").length;
    for (const selector of splitSelectors(rule.header)) {
      const classNames = [...selector.matchAll(/\.([A-Za-z_][A-Za-z0-9_-]*)/g)].map((match) => match[1]);
      const missing = classNames.filter((className) => !sourceMentionsClass(className) && !isDynamicClass(className));
      if (missing.length === 0) continue;
      candidates += 1;
      console.log(`${relative(appRoot, file)}:${line}\t${selector}\tmissing: ${missing.join(", ")}`);
    }
  }
}

console.log(`\n${candidates} selector candidate(s); inspect each before deleting.`);
if (candidates > 0) process.exitCode = 1;
