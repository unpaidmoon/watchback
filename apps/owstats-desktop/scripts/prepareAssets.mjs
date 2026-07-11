import { copyFile, mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const sensitiveTerms = ["token", "cookie", "authorization", "credential", "storage"];

function normalizedRelativePath(value, manifestName) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${manifestName}: 资源路径必须是非空字符串`);
  }
  const slashPath = value.replaceAll("\\", "/");
  if (slashPath.includes("/@fs/") || slashPath.startsWith("/@fs")) {
    throw new Error(`${manifestName}: 禁止 /@fs/ 路径 ${value}`);
  }
  if (path.posix.isAbsolute(slashPath) || path.win32.isAbsolute(value)) {
    throw new Error(`${manifestName}: 禁止绝对路径 ${value}`);
  }
  if (slashPath.split("/").includes("..")) {
    throw new Error(`${manifestName}: 禁止 .. 越界路径 ${value}`);
  }
  const normalized = path.posix.normalize(slashPath).replace(/^\.\//, "");
  if (!normalized || normalized === ".") {
    throw new Error(`${manifestName}: 资源路径不能为空`);
  }
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

function collectLocalPaths(value, result = []) {
  if (Array.isArray(value)) {
    value.forEach((item) => collectLocalPaths(item, result));
  } else if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      if (key === "local_path" || key === "localPath") result.push(child);
      collectLocalPaths(child, result);
    }
  }
  return result;
}

async function copyChecked(source, destination, manifestName, referencedPath) {
  try {
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
  } catch (error) {
    throw new Error(
      `${manifestName}: 引用文件不存在 ${referencedPath} (${error instanceof Error ? error.message : String(error)})`,
    );
  }
}

export async function prepareAssets({ repoRoot, appRoot } = {}) {
  if (!repoRoot || !appRoot) throw new Error("prepareAssets 需要 repoRoot 和 appRoot");
  const assetsRoot = path.resolve(repoRoot, "assets");
  const outputRoot = path.resolve(appRoot, "public", "assets");
  const stagingRoot = `${outputRoot}.tmp`;
  const manifests = {
    root: { file: path.join(assetsRoot, "manifest.json"), name: "assets/manifest.json" },
    ranks: { file: path.join(assetsRoot, "ranks", "manifest.json"), name: "assets/ranks/manifest.json" },
    stadium: { file: path.join(assetsRoot, "stadium", "manifest.json"), name: "assets/stadium/manifest.json" },
  };
  const runtimeFile = path.resolve(appRoot, "runtime-config", "ow-lookups.json");
  const runtimeName = "runtime-config/ow-lookups.json";

  const rootManifest = await readJson(manifests.root.file, manifests.root.name);
  const ranksManifest = await readJson(manifests.ranks.file, manifests.ranks.name);
  const stadiumManifest = await readJson(manifests.stadium.file, manifests.stadium.name);
  const runtimeConfig = await readJson(runtimeFile, runtimeName);
  assertNoSensitiveKeys(runtimeConfig, runtimeName);

  const copies = [];
  for (const rawPath of collectLocalPaths(rootManifest)) {
    const relative = normalizedRelativePath(rawPath, manifests.root.name);
    const prefix = "data/assets/overwatch/";
    if (!relative.startsWith(prefix)) throw new Error(`${manifests.root.name}: 路径不在允许的源目录内 ${rawPath}`);
    const assetPath = relative.slice(prefix.length);
    copies.push([path.join(assetsRoot, ...assetPath.split("/")), path.join(stagingRoot, ...assetPath.split("/")), manifests.root.name, rawPath]);
  }
  for (const rank of ranksManifest.ranks ?? []) {
    const relative = normalizedRelativePath(rank.file, manifests.ranks.name);
    copies.push([path.join(assetsRoot, "ranks", ...relative.split("/")), path.join(stagingRoot, "ranks", ...relative.split("/")), manifests.ranks.name, rank.file]);
  }
  for (const rawPath of ranksManifest.openWideFiles ?? []) {
    const relative = normalizedRelativePath(rawPath, manifests.ranks.name);
    copies.push([path.join(assetsRoot, "ranks", ...relative.split("/")), path.join(stagingRoot, "ranks", ...relative.split("/")), manifests.ranks.name, rawPath]);
  }
  for (const rawPath of collectLocalPaths(stadiumManifest)) {
    const relative = normalizedRelativePath(rawPath, manifests.stadium.name);
    const prefix = "assets/";
    if (!relative.startsWith(prefix)) throw new Error(`${manifests.stadium.name}: 路径不在允许的源目录内 ${rawPath}`);
    const assetPath = relative.slice(prefix.length);
    copies.push([path.join(assetsRoot, ...assetPath.split("/")), path.join(stagingRoot, ...assetPath.split("/")), manifests.stadium.name, rawPath]);
  }

  await rm(stagingRoot, { recursive: true, force: true });
  await mkdir(stagingRoot, { recursive: true });
  try {
    for (const copy of copies) await copyChecked(...copy);
    await copyChecked(manifests.root.file, path.join(stagingRoot, "manifest.json"), manifests.root.name, "manifest.json");
    await copyChecked(manifests.ranks.file, path.join(stagingRoot, "ranks", "manifest.json"), manifests.ranks.name, "manifest.json");
    await copyChecked(manifests.stadium.file, path.join(stagingRoot, "stadium", "manifest.json"), manifests.stadium.name, "manifest.json");
    await mkdir(path.join(stagingRoot, "config"), { recursive: true });
    await writeFile(path.join(stagingRoot, "config", "ow-lookups.json"), `${JSON.stringify(runtimeConfig, null, 2)}\n`);
    await rm(outputRoot, { recursive: true, force: true });
    await rename(stagingRoot, outputRoot);
  } catch (error) {
    await rm(stagingRoot, { recursive: true, force: true });
    throw error;
  }
  return { outputRoot, fileCount: copies.length + 4 };
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
