import { randomUUID } from "node:crypto";
import { execFile } from "node:child_process";
import { copyFile, lstat, mkdir, readFile, readdir, realpath, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const sensitiveTerms = ["token", "cookie", "authorization", "credential", "storage"];
const execFileAsync = promisify(execFile);

function normalizedRelativePath(value, manifestName) {
  if (typeof value !== "string" || value.length === 0) throw new Error(`${manifestName}: 资源路径必须是非空字符串`);
  const slashPath = value.replaceAll("\\", "/");
  if (slashPath.includes("/@fs/") || slashPath.startsWith("/@fs")) {
    throw new Error(`${manifestName}: 禁止 /@fs/ 路径 ${value}`);
  }
  if (path.posix.isAbsolute(slashPath) || path.win32.isAbsolute(value)) {
    throw new Error(`${manifestName}: 禁止绝对路径 ${value}`);
  }
  if (slashPath.split("/").includes("..")) throw new Error(`${manifestName}: 禁止 .. 越界路径 ${value}`);
  const normalized = path.posix.normalize(slashPath).replace(/^\.\//, "");
  if (!normalized || normalized === ".") throw new Error(`${manifestName}: 资源路径不能为空`);
  return normalized;
}

function assertNoSensitiveKeys(value, configName, jsonPath = "root") {
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertNoSensitiveKeys(item, configName, `${jsonPath}[${index}]`));
    return;
  }
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    const normalizedKey = key.toLowerCase().replace(/[^a-z0-9]/g, "");
    if (sensitiveTerms.some((term) => normalizedKey.includes(term))) {
      throw new Error(`${configName}: 检测到敏感字段 ${jsonPath}.${key}`);
    }
    assertNoSensitiveKeys(child, configName, `${jsonPath}.${key}`);
  }
}

async function readJson(file, displayName) {
  try {
    return JSON.parse(await readFile(file, "utf8"));
  } catch (error) {
    throw new Error(`${displayName}: 无法解析 JSON (${error instanceof Error ? error.message : String(error)})`);
  }
}

function assertObject(value, name) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${name}: 根结构必须是对象`);
}

function assertArrayField(value, field, name) {
  if (!Array.isArray(value[field])) throw new Error(`${name}: ${field} 必须是数组`);
}

function assertObjectEntries(entries, field, name) {
  entries.forEach((entry, index) => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
      throw new Error(`${name}: ${field}[${index}] 必须是对象`);
    }
  });
}

function collectFields(value, fieldNames, result = []) {
  if (Array.isArray(value)) value.forEach((item) => collectFields(item, fieldNames, result));
  else if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      if (fieldNames.has(key)) result.push({ field: key, value: child });
      collectFields(child, fieldNames, result);
    }
  }
  return result;
}

function isInsideRoot(candidate, root) {
  const relative = path.relative(root, candidate);
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative));
}

async function assertSafeSourceFile(file, allowedRoot, displayName) {
  const absoluteRoot = path.resolve(allowedRoot);
  const absoluteFile = path.resolve(file);
  if (!isInsideRoot(absoluteFile, absoluteRoot)) throw new Error(`${displayName}: 路径越过允许根目录`);
  const rootReal = await realpath(absoluteRoot);
  let current = absoluteRoot;
  for (const part of path.relative(absoluteRoot, absoluteFile).split(path.sep).filter(Boolean)) {
    current = path.join(current, part);
    const info = await lstat(current);
    if (info.isSymbolicLink()) throw new Error(`禁止符号链接 ${displayName}`);
  }
  const info = await lstat(absoluteFile);
  if (!info.isFile()) throw new Error(`${displayName}: 必须是普通文件`);
  const fileReal = await realpath(absoluteFile);
  if (!isInsideRoot(fileReal, rootReal)) throw new Error(`${displayName}: realpath 越过允许根目录`);
}

async function assertSafeSourceRoot(root, allowedParent, displayName) {
  const absoluteRoot = path.resolve(root);
  const absoluteParent = path.resolve(allowedParent);
  const rootInfo = await lstat(absoluteRoot);
  if (rootInfo.isSymbolicLink()) throw new Error(`${displayName} 根目录禁止符号链接或 junction`);
  if (!rootInfo.isDirectory()) throw new Error(`${displayName} 根目录必须是目录`);
  const parentReal = await realpath(absoluteParent);
  const rootReal = await realpath(absoluteRoot);
  if (!isInsideRoot(rootReal, parentReal)) throw new Error(`${displayName} 根目录 realpath 越过允许目录`);
}

async function assertReferencedFile(source, allowedRoot, manifestName, field, rawPath) {
  try {
    await assertSafeSourceFile(source, allowedRoot, `${manifestName} ${field} ${rawPath}`);
  } catch (error) {
    throw new Error(`${manifestName}: ${field} 引用文件无效 ${rawPath} (${error instanceof Error ? error.message : String(error)})`);
  }
}

async function validateManifests({ rootManifest, ranksManifest, stadiumManifest, assetsRoot, inventoryFiles }) {
  const inventorySet = new Set(inventoryFiles);
  const assertInventoried = (relative, reference) => {
    if (!inventorySet.has(relative)) {
      throw new Error(`runtime-config/assets-inventory.json: 未列出 ${reference} 引用 ${relative}`);
    }
  };
  assertObject(rootManifest, "assets/manifest.json");
  for (const field of ["heroes", "maps", "perks"]) {
    assertArrayField(rootManifest, field, "assets/manifest.json");
    assertObjectEntries(rootManifest[field], field, "assets/manifest.json");
  }
  if ("achievements" in rootManifest) {
    assertArrayField(rootManifest, "achievements", "assets/manifest.json");
    assertObjectEntries(rootManifest.achievements, "achievements", "assets/manifest.json");
  }
  for (const { field, value } of collectFields(rootManifest, new Set(["local_path"]))) {
    const relative = normalizedRelativePath(value, "assets/manifest.json");
    const prefix = "data/assets/overwatch/";
    if (!relative.startsWith(prefix)) throw new Error(`assets/manifest.json: ${field} 路径不在允许的源目录内 ${value}`);
    const assetPath = relative.slice(prefix.length);
    assertInventoried(assetPath, `assets/manifest.json ${field}`);
    await assertReferencedFile(path.join(assetsRoot, ...assetPath.split("/")), assetsRoot, "assets/manifest.json", field, value);
  }

  assertObject(ranksManifest, "assets/ranks/manifest.json");
  assertArrayField(ranksManifest, "ranks", "assets/ranks/manifest.json");
  assertArrayField(ranksManifest, "openWideFiles", "assets/ranks/manifest.json");
  assertObjectEntries(ranksManifest.ranks, "ranks", "assets/ranks/manifest.json");
  for (const { field, value } of collectFields(ranksManifest.ranks, new Set(["file", "wideFile"]))) {
    const relative = normalizedRelativePath(value, "assets/ranks/manifest.json");
    assertInventoried(`ranks/${relative}`, `assets/ranks/manifest.json ${field}`);
    await assertReferencedFile(path.join(assetsRoot, "ranks", ...relative.split("/")), path.join(assetsRoot, "ranks"), "assets/ranks/manifest.json", field, value);
  }
  for (const value of ranksManifest.openWideFiles) {
    const relative = normalizedRelativePath(value, "assets/ranks/manifest.json");
    assertInventoried(`ranks/${relative}`, "assets/ranks/manifest.json openWideFiles");
    await assertReferencedFile(path.join(assetsRoot, "ranks", ...relative.split("/")), path.join(assetsRoot, "ranks"), "assets/ranks/manifest.json", "openWideFiles", value);
  }

  assertObject(stadiumManifest, "assets/stadium/manifest.json");
  for (const field of ["fightRanks", "modTrait"]) {
    assertArrayField(stadiumManifest, field, "assets/stadium/manifest.json");
    assertObjectEntries(stadiumManifest[field], field, "assets/stadium/manifest.json");
  }
  for (const { field, value } of collectFields(stadiumManifest, new Set(["localPath", "pureLocalPath"]))) {
    const relative = normalizedRelativePath(value, "assets/stadium/manifest.json");
    const prefix = "assets/";
    if (!relative.startsWith(prefix)) throw new Error(`assets/stadium/manifest.json: ${field} 路径不在允许的源目录内 ${value}`);
    const assetPath = relative.slice(prefix.length);
    assertInventoried(assetPath, `assets/stadium/manifest.json ${field}`);
    await assertReferencedFile(path.join(assetsRoot, ...assetPath.split("/")), assetsRoot, "assets/stadium/manifest.json", field, value);
  }
}

function validateInventory(inventory) {
  const inventoryName = "runtime-config/assets-inventory.json";
  assertObject(inventory, inventoryName);
  if (inventory.version !== 1) throw new Error(`${inventoryName}: version 必须是 1`);
  assertArrayField(inventory, "files", inventoryName);
  const normalized = inventory.files.map((value) => normalizedRelativePath(value, inventoryName));
  const unique = new Set(normalized);
  if (unique.size !== normalized.length) throw new Error(`${inventoryName}: files 包含重复路径`);
  const sorted = [...normalized].sort((left, right) => left.localeCompare(right));
  if (!normalized.every((value, index) => value === sorted[index])) throw new Error(`${inventoryName}: files 必须排序`);
  return normalized;
}

async function copyInventoryFiles(assetsRoot, stagingRoot, inventoryFiles) {
  for (const relative of inventoryFiles) {
    const source = path.join(assetsRoot, ...relative.split("/"));
    try {
      await assertSafeSourceFile(source, assetsRoot, `assets-inventory.json ${relative}`);
    } catch (error) {
      throw new Error(
        `runtime-config/assets-inventory.json: ${relative} 不存在或不是安全普通文件 (${error instanceof Error ? error.message : String(error)})`,
      );
    }
    const destination = path.join(stagingRoot, ...relative.split("/"));
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
  }
}

export async function replaceDirectoryAtomically(outputRoot, stagingRoot, injected = {}) {
  const fsOps = { rename, rm, lstat, ...injected };
  const backupRoot = `${outputRoot}.backup-${randomUUID()}`;
  let oldMoved = false;
  try {
    if (await pathExistsWith(fsOps, outputRoot)) {
      await fsOps.rename(outputRoot, backupRoot);
      oldMoved = true;
    }
    try {
      await fsOps.rename(stagingRoot, outputRoot);
    } catch (error) {
      await fsOps.rm(outputRoot, { recursive: true, force: true });
      if (oldMoved) await fsOps.rename(backupRoot, outputRoot);
      throw error;
    }
    if (oldMoved) await fsOps.rm(backupRoot, { recursive: true, force: true });
  } finally {
    await fsOps.rm(stagingRoot, { recursive: true, force: true });
  }
}

async function pathExistsWith(fsOps, target) {
  try {
    await fsOps.lstat(target);
    return true;
  } catch (error) {
    if (error?.code === "ENOENT") return false;
    throw error;
  }
}

export async function prepareAssets({ repoRoot, appRoot } = {}) {
  if (!repoRoot || !appRoot) throw new Error("prepareAssets 需要 repoRoot 和 appRoot");
  const assetsRoot = path.resolve(repoRoot, "assets");
  const outputRoot = path.resolve(appRoot, "public", "assets");
  const stagingRoot = `${outputRoot}.stage-${randomUUID()}`;
  const rootManifestFile = path.join(assetsRoot, "manifest.json");
  const ranksManifestFile = path.join(assetsRoot, "ranks", "manifest.json");
  const stadiumManifestFile = path.join(assetsRoot, "stadium", "manifest.json");
  const runtimeRoot = path.resolve(appRoot, "runtime-config");
  const runtimeFile = path.join(runtimeRoot, "ow-lookups.json");
  const inventoryFile = path.join(runtimeRoot, "assets-inventory.json");

  await assertSafeSourceRoot(assetsRoot, repoRoot, "assets");
  await assertSafeSourceRoot(runtimeRoot, appRoot, "runtime-config");
  await assertSafeSourceFile(rootManifestFile, assetsRoot, "assets/manifest.json");
  await assertSafeSourceFile(ranksManifestFile, assetsRoot, "assets/ranks/manifest.json");
  await assertSafeSourceFile(stadiumManifestFile, assetsRoot, "assets/stadium/manifest.json");
  await assertSafeSourceFile(runtimeFile, runtimeRoot, "runtime-config/ow-lookups.json");
  await assertSafeSourceFile(inventoryFile, runtimeRoot, "runtime-config/assets-inventory.json");
  const rootManifest = await readJson(rootManifestFile, "assets/manifest.json");
  const ranksManifest = await readJson(ranksManifestFile, "assets/ranks/manifest.json");
  const stadiumManifest = await readJson(stadiumManifestFile, "assets/stadium/manifest.json");
  const runtimeConfig = await readJson(runtimeFile, "runtime-config/ow-lookups.json");
  const inventory = await readJson(inventoryFile, "runtime-config/assets-inventory.json");
  assertNoSensitiveKeys(runtimeConfig, "runtime-config/ow-lookups.json");
  const inventoryFiles = validateInventory(inventory);
  await validateManifests({ rootManifest, ranksManifest, stadiumManifest, assetsRoot, inventoryFiles });

  await mkdir(stagingRoot, { recursive: true });
  try {
    await copyInventoryFiles(assetsRoot, stagingRoot, inventoryFiles);
    await replaceDirectoryAtomically(outputRoot, stagingRoot);
  } catch (error) {
    await rm(stagingRoot, { recursive: true, force: true });
    throw error;
  }
  return { outputRoot, fileCount: (await collectFiles(outputRoot)).length };
}

export async function refreshAssetsInventory({ repoRoot, appRoot }) {
  const { stdout } = await execFileAsync("git", ["-C", repoRoot, "ls-files", "--", "assets"]);
  const files = stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map((file) => file.replaceAll("\\", "/"))
    .filter((file) => file.startsWith("assets/"))
    .map((file) => file.slice("assets/".length));
  const uniqueSorted = [...new Set(files)].sort((left, right) => left.localeCompare(right));
  const inventoryFile = path.resolve(appRoot, "runtime-config", "assets-inventory.json");
  await writeFile(inventoryFile, `${JSON.stringify({ version: 1, files: uniqueSorted }, null, 2)}\n`);
  return { inventoryFile, fileCount: uniqueSorted.length };
}

async function collectFiles(root) {
  const files = [];
  async function walk(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const item = path.join(directory, entry.name);
      if (entry.isDirectory()) await walk(item);
      else files.push(item);
    }
  }
  await walk(root);
  return files;
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  const appRoot = path.resolve(path.dirname(scriptPath), "..");
  const repoRoot = path.resolve(appRoot, "..", "..");
  if (process.argv.includes("--refresh-inventory")) {
    refreshAssetsInventory({ repoRoot, appRoot })
      .then(({ inventoryFile, fileCount }) => console.log(`已刷新 ${fileCount} 个 tracked assets：${inventoryFile}`))
      .catch((error) => {
        console.error(error instanceof Error ? error.message : String(error));
        process.exitCode = 1;
      });
  } else {
    prepareAssets({ repoRoot, appRoot })
      .then(({ outputRoot, fileCount }) => console.log(`已准备 ${fileCount} 个文件：${outputRoot}`))
      .catch((error) => {
        console.error(error instanceof Error ? error.message : String(error));
        process.exitCode = 1;
      });
  }
}
