import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { basename, extname, relative, resolve } from "node:path";

const ignoredDirectories = new Set([".git", "node_modules", "target", "dist", "docs/reference", "data", "chrome-dashen-profile"]);
const ignoredTestPaths = ["/tests/", ".test.", "/fixtures/", "/docs/superpowers/", "/AGENTS.md"];
const knownSafeSourcePaths = new Set([
  "apps/owstats-desktop/scripts/prepareAssets.mjs",
  "apps/owstats-desktop/scripts/verifyDistPaths.mjs",
]);
const textExtensions = new Set([".cjs", ".css", ".html", ".json", ".js", ".md", ".mjs", ".ps1", ".rs", ".toml", ".ts", ".tsx", ".yml", ".yaml"]);

function trackedFiles(root) {
  try {
    const output = execFileSync("git", ["-C", root, "ls-files", "-z"], { encoding: "buffer" });
    const files = output.toString("utf8").split("\0").filter(Boolean).map((path) => resolve(root, path));
    return files.length ? files : walk(root);
  } catch {
    return walk(root);
  }
}

function walk(root) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (!ignoredDirectories.has(entry.name)) files.push(...walk(resolve(root, entry.name)));
    } else if (entry.isFile()) {
      files.push(resolve(root, entry.name));
    }
  }
  return files;
}

function isTextFile(path) {
  return textExtensions.has(extname(path).toLowerCase()) || basename(path) === "AGENTS.md";
}

function ignored(relativePath, scanFixtureRoot) {
  const normalized = `/${relativePath.replaceAll("\\", "/")}`;
  const projectRelative = normalized.slice(1);
  return (
    normalized.startsWith("/docs/") ||
    knownSafeSourcePaths.has(projectRelative) ||
    ignoredTestPaths.some((part) => normalized.includes(part) && (part !== "/fixtures/" || !scanFixtureRoot))
  );
}

function findingsForText(text, relativePath, scanFixtureRoot) {
  const findings = [];
  const rules = [
    ["absolute-path", /(?:D:[\\/]Codex[\\/]|C:[\\/]Users[\\/]|\/@fs\/)/g],
    ["sensitive-key", /["'](?:customer[_-]?token|access[_-]?token|token|cookie|authorization|credential|storage)["']\s*:\s*["'](?!\*\*\*REDACTED\*\*\*)[^"']+/gi],
  ];
  for (const [rule, pattern] of rules) {
    if (rule === "sensitive-key" && basename(relativePath) === "package-lock.json") continue;
    for (const match of text.matchAll(pattern)) {
      const before = text.slice(0, match.index);
      const line = before.split("\n").length;
      const value = match[0].toLowerCase();
      if (!scanFixtureRoot && rule === "sensitive-key" && /(synthetic|legacy|token-secret|customer-secret)/.test(value)) continue;
      findings.push({ relativePath, line, rule });
    }
  }
  return findings;
}

export function scanRoot(root, { includeDist = false } = {}) {
  const resolvedRoot = resolve(root);
  const scanFixtureRoot = resolvedRoot.replaceAll("\\", "/").includes("/scripts/fixtures/verify/");
  const candidates = trackedFiles(resolvedRoot);
  if (includeDist && existsSync(resolve(resolvedRoot, "apps/owstats-desktop/dist"))) {
    candidates.push(...walk(resolve(resolvedRoot, "apps/owstats-desktop/dist")));
  }
  return candidates.flatMap((path) => {
    const rel = relative(resolvedRoot, path);
    if (!isTextFile(path) || ignored(rel, scanFixtureRoot)) return [];
    const content = readFileSync(path, "utf8");
    return findingsForText(content, rel, scanFixtureRoot);
  });
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const rootIndex = args.indexOf("--root");
  const root = rootIndex >= 0 ? args[rootIndex + 1] : process.cwd();
  if (rootIndex >= 0 && !root) throw new Error("--root 需要目录参数");
  const findings = scanRoot(root, { includeDist: args.includes("--include-dist") });
  for (const finding of findings) {
    console.error(`${finding.relativePath}:${finding.line}:${finding.rule}:<已脱敏>`);
  }
  process.exitCode = findings.length ? 1 : 0;
}
