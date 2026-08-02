import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  detectRouterSecurityViolations,
  isExecutableSourceFile,
  isRootBuildConfig,
} from "./routerSecurityPolicyDetector.mjs";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const policy = JSON.parse(
  await readFile(path.join(appRoot, "scripts", "dependencySecurityPolicy.json"), "utf8"),
);
const exception = policy.reactRouterException;
const packageJson = JSON.parse(await readFile(path.join(appRoot, "package.json"), "utf8"));
const packageLock = JSON.parse(await readFile(path.join(appRoot, "package-lock.json"), "utf8"));

assert.equal(policy.schemaVersion, 1, "unsupported dependency security policy schema");
assert.match(exception.expiresOn, /^\d{4}-\d{2}-\d{2}$/);
assert.ok(exception.rationale.trim(), "security exception rationale is required");
assert.ok(exception.removalCondition.trim(), "security exception removal condition is required");

for (const [dependency, version] of Object.entries(exception.exactVersions)) {
  if (dependency === "react-router-dom") {
    assert.equal(packageJson.dependencies[dependency], version, `${dependency} must remain exactly pinned`);
    assert.equal(packageLock.packages[""].dependencies?.[dependency], version);
  }
  assert.equal(packageLock.packages[`node_modules/${dependency}`].version, version);
}
assert.equal(
  packageLock.packages["node_modules/react-router-dom"].dependencies?.["react-router"],
  exception.exactVersions["react-router"],
);

const excludedScannerFiles = new Set([
  "scripts/routerSecurityPolicyDetector.mjs",
  "scripts/routerSecurityPolicyDetector.test.ts",
]);

async function collectExecutableFiles(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(entries.map(async (entry) => {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) return collectExecutableFiles(target);
    return isExecutableSourceFile(entry.name) ? [target] : [];
  }));
  return nested.flat();
}

const candidates = [
  ...await collectExecutableFiles(path.join(appRoot, "src")),
  ...await collectExecutableFiles(path.join(appRoot, "scripts")),
];
for (const entry of await readdir(appRoot, { withFileTypes: true })) {
  if (entry.isFile() && isRootBuildConfig(entry.name)) candidates.push(path.join(appRoot, entry.name));
}

for (const file of candidates) {
  const relative = path.relative(appRoot, file).replaceAll("\\", "/");
  if (excludedScannerFiles.has(relative)) continue;
  const violations = detectRouterSecurityViolations(
    await readFile(file, "utf8"),
    policy.reactRouterBoundary,
  );
  assert.deepEqual(violations, [], `${relative}: ${violations.join(", ")}`);
}

console.log(`React Router security policy passed (${candidates.length - excludedScannerFiles.size} files scanned)`);
