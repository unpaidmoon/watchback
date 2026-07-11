import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
    { label: "D:/Codex", pattern: /D:\/Codex(?:\/|\\)/i },
    { label: "C:\\Users", pattern: /C:\\Users\\/i },
    { label: "docs/reference", pattern: /docs[\\/]reference/i },
    { label: "repo absolute path", pattern: new RegExp(normalizedRepoRoot.replace(/[.*+?^${}()|[\]\\]/g, "\\$&").replaceAll("/", "[\\\\/]"), "i") },
  ];

  for (const file of await collectFiles(distRoot)) {
    const content = (await readFile(file)).toString("utf8");
    const match = forbidden.find(({ pattern }) => pattern.test(content));
    if (match) throw new Error(`${path.relative(distRoot, file)}: 构建产物包含禁用路径 ${match.label}`);
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
