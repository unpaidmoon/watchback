import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
const repoRoot = resolve(appRoot, "../..");
const scanRoots = [
  { base: repoRoot, path: "README.md" },
  { base: repoRoot, path: "docs" },
  { base: appRoot, path: "src" },
  { base: appRoot, path: "scripts" },
  { base: appRoot, path: "src-tauri/src" },
  { base: appRoot, path: "src-tauri/tests" },
];
const ignoredDirs = new Set(["node_modules", "target", "dist", ".git", "superpowers"]);
const extensions = new Set([".md", ".ts", ".tsx", ".rs", ".json"]);
const mojibakePatterns = [
  /\u93c4|\u7487|\u93c8|\u7035|\u9427|\u935a|\u74a7|\u752f|\u68f0|\u9225|\ufffd/,
  /\u9286|\u20ac\?|\u6d93|\u7edb|\u59dd|\u5997|\u9366|\u701b/,
];

function hasScannedExtension(path) {
  return [...extensions].some((extension) => path.endsWith(extension));
}

function walk(base, path, files = []) {
  const fullPath = join(base, path);
  const stat = statSync(fullPath, { throwIfNoEntry: false });

  if (!stat) {
    return files;
  }

  if (stat.isDirectory()) {
    if (ignoredDirs.has(path.split(/[\\/]/).pop())) {
      return files;
    }

    for (const child of readdirSync(fullPath)) {
      walk(base, join(path, child), files);
    }

    return files;
  }

  if (hasScannedExtension(path)) {
    files.push(fullPath);
  }

  return files;
}

const failures = [];

for (const entry of scanRoots) {
  for (const file of walk(entry.base, entry.path)) {
    const text = readFileSync(file, "utf8");
    const lines = text.split(/\r?\n/);

    lines.forEach((line, index) => {
      if (mojibakePatterns.some((pattern) => pattern.test(line))) {
        failures.push(`${relative(repoRoot, file)}:${index + 1}: ${line.trim()}`);
      }
    });
  }
}

if (failures.length > 0) {
  console.error("Mojibake-like text found:");
  console.error(failures.join("\n"));
  process.exit(1);
}
