import { randomUUID } from "node:crypto";
import { copyFile, lstat, mkdir, readFile, readdir, realpath, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const sensitiveTerms = ["token", "cookie", "authorization", "credential", "storage"];

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

async function assertReferencedFile(source, allowedRoot, manifestName, field, rawPath) {
  try {
    await assertSafeSourceFile(source, allowedRoot, `${manifestName} ${field} ${rawPath}`);
  } catch (error) {
    throw new Error(`${manifestName}: ${field} 引用文件无效 ${rawPath} (${error instanceof Error ? error.message : String(error)})`);
  }
}

async function validateManifests({ rootManifest, ranksManifest, stadiumManifest, assetsRoot }) {
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
    await assertReferencedFile(path.join(assetsRoot, ...assetPath.split("/")), assetsRoot, "assets/manifest.json", field, value);
  }

  assertObject(ranksManifest, "assets/ranks/manifest.json");
  assertArrayField(ranksManifest, "ranks", "assets/ranks/manifest.json");
  assertArrayField(ranksManifest, "openWideFiles", "assets/ranks/manifest.json");
  assertObjectEntries(ranksManifest.ranks, "ranks", "assets/ranks/manifest.json");
  for (const { field, value } of collectFields(ranksManifest.ranks, new Set(["file", "wideFile"]))) {
    const relative = normalizedRelativePath(value, "assets/ranks/manifest.json");
    await assertReferencedFile(path.join(assetsRoot, "ranks", ...relative.split("/")), path.join(assetsRoot, "ranks"), "assets/ranks/manifest.json", field, value);
  }
  for (const value of ranksManifest.openWideFiles) {
    const relative = normalizedRelativePath(value, "assets/ranks/manifest.json");
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
    await assertReferencedFile(path.join(assetsRoot, ...assetPath.split("/")), assetsRoot, "assets/stadium/manifest.json", field, value);
  }
}

async function copySourceTree(sourceRoot, destinationRoot) {
  async function walk(source, destination) {
    const sourceInfo = await lstat(source);
    if (sourceInfo.isSymbolicLink()) throw new Error(`禁止符号链接 ${path.relative(sourceRoot, source)}`);
    if (!sourceInfo.isDirectory()) throw new Error(`${source}: 源根目录必须是目录`);
    await mkdir(destination, { recursive: true });
    const entries = await readdir(source, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const sourcePath = path.join(source, entry.name);
      const destinationPath = path.join(destination, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`禁止符号链接 ${path.relative(sourceRoot, sourcePath)}`);
      if (entry.isDirectory()) await walk(sourcePath, destinationPath);
      else {
        await assertSafeSourceFile(sourcePath, sourceRoot, path.relative(sourceRoot, sourcePath));
        await mkdir(path.dirname(destinationPath), { recursive: true });
        await copyFile(sourcePath, destinationPath);
      }
    }
  }
  await walk(sourceRoot, destinationRoot);
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

  await assertSafeSourceFile(rootManifestFile, assetsRoot, "assets/manifest.json");
  await assertSafeSourceFile(ranksManifestFile, assetsRoot, "assets/ranks/manifest.json");
  await assertSafeSourceFile(stadiumManifestFile, assetsRoot, "assets/stadium/manifest.json");
  await assertSafeSourceFile(runtimeFile, runtimeRoot, "runtime-config/ow-lookups.json");
  const rootManifest = await readJson(rootManifestFile, "assets/manifest.json");
  const ranksManifest = await readJson(ranksManifestFile, "assets/ranks/manifest.json");
  const stadiumManifest = await readJson(stadiumManifestFile, "assets/stadium/manifest.json");
  const runtimeConfig = await readJson(runtimeFile, "runtime-config/ow-lookups.json");
  assertNoSensitiveKeys(runtimeConfig, "runtime-config/ow-lookups.json");
  await validateManifests({ rootManifest, ranksManifest, stadiumManifest, assetsRoot });

  await mkdir(stagingRoot, { recursive: true });
  try {
    await copySourceTree(assetsRoot, stagingRoot);
    await mkdir(path.join(stagingRoot, "config"), { recursive: true });
    await writeFile(path.join(stagingRoot, "config", "ow-lookups.json"), `${JSON.stringify(runtimeConfig, null, 2)}\n`);
    await replaceDirectoryAtomically(outputRoot, stagingRoot);
  } catch (error) {
    await rm(stagingRoot, { recursive: true, force: true });
    throw error;
  }
  return { outputRoot, fileCount: (await collectFiles(outputRoot)).length };
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
  prepareAssets({ repoRoot, appRoot })
    .then(({ outputRoot, fileCount }) => console.log(`已准备 ${fileCount} 个文件：${outputRoot}`))
    .catch((error) => {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    });
}
