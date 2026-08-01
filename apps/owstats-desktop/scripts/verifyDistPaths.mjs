import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const textExtensions = new Set([".css", ".html", ".js", ".json", ".map", ".mjs", ".svg", ".txt", ".xml"]);
const maxJavaScriptBytes = 500_000;
const maxPhase6ComponentsBytes = 250_000;

async function collectFiles(root) {
  const files = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const candidate = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...(await collectFiles(candidate)));
    else if (entry.isFile()) files.push(candidate);
  }
  return files;
}

export async function verifyDistPaths(distRoot, { repoRoot = path.resolve(distRoot, "..", "..", "..") } = {}) {
  const normalizedRepoRoot = path.resolve(repoRoot).replaceAll("\\", "/");
  const forbidden = [
    { label: "/@fs/", pattern: /\/@fs\//i },
    { label: "Windows absolute path", pattern: /(?:^|[^A-Z0-9])[A-Z]:[\\/]/i },
    { label: "docs/reference", pattern: /docs[\\/]reference/i },
    { label: "repo absolute path", pattern: new RegExp(normalizedRepoRoot.replace(/[.*+?^${}()|[\]\\]/g, "\\$&").replaceAll("/", "[\\\\/]"), "i") },
  ];

  for (const file of await collectFiles(distRoot)) {
    if (!textExtensions.has(path.extname(file).toLowerCase())) continue;
    const data = await readFile(file);
    const relativeFile = path.relative(distRoot, file);
    if (path.extname(file).toLowerCase() === ".js") {
      const limit = path.basename(file).startsWith("phase6Components-")
        ? maxPhase6ComponentsBytes
        : maxJavaScriptBytes;
      if (data.byteLength > limit) {
        throw new Error(`${relativeFile}: JavaScript 分块 ${data.byteLength} B 超过预算 ${limit} B`);
      }
    }
    const content = data.toString("utf8");
    const match = forbidden.find(({ pattern }) => pattern.test(content));
    if (match) throw new Error(`${relativeFile}: 构建产物包含禁用路径 ${match.label}`);
  }
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  const appRoot = path.resolve(path.dirname(scriptPath), "..");
  verifyDistPaths(path.join(appRoot, "dist"), { repoRoot: path.resolve(appRoot, "..", "..") })
    .then(() => console.log("dist 路径扫描通过"))
    .catch((error) => {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    });
}
