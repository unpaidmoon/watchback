import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(await readFile(path.join(appRoot, "package.json"), "utf8")) as {
  dependencies: Record<string, string>;
};
const packageLock = JSON.parse(await readFile(path.join(appRoot, "package-lock.json"), "utf8")) as {
  packages: Record<string, { version?: string; dependencies?: Record<string, string> }>;
};

assert.equal(
  packageJson.dependencies["react-router-dom"],
  "7.18.2",
  "React Router 临时安全例外只允许精确版本 7.18.2；升级时必须显式更新安全策略",
);
assert.equal(packageLock.packages[""].dependencies?.["react-router-dom"], "7.18.2");
assert.equal(packageLock.packages["node_modules/react-router-dom"].version, "7.18.2");
assert.equal(packageLock.packages["node_modules/react-router-dom"].dependencies?.["react-router"], "7.18.2");
assert.equal(packageLock.packages["node_modules/react-router"].version, "7.18.2");

async function sourceFiles(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = await Promise.all(entries.map(async (entry) => {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) return sourceFiles(target);
    return /\.(?:ts|tsx|js|jsx|mjs|cjs)$/.test(entry.name) ? [target] : [];
  }));
  return files.flat();
}

const forbidden = [
  /from\s+["']react-router\/dom\/server["']/,
  /from\s+["']react-router\/rsc(?:\/[^"']*)?["']/,
  /from\s+["']react-router-dom\/server["']/,
  /\bentry\.rsc\b/i,
  /\bRSCStaticRouter\b/,
  /\brouteRSCServerRequest\b/,
  /\bdecodeAction\b/,
  /\bdecodeFormState\b/,
];

for (const file of await sourceFiles(path.join(appRoot, "src"))) {
  const source = await readFile(file, "utf8");
  for (const pattern of forbidden) {
    assert.doesNotMatch(source, pattern, `${path.relative(appRoot, file)} 引入了会使安全例外失效的 RSC/服务端 Router API`);
  }
}

console.log("React Router 安全策略契约通过");
