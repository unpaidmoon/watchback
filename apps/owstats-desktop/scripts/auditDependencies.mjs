import { spawnSync } from "node:child_process";

const allowedAdvisory = "https://github.com/advisories/GHSA-qwww-vcr4-c8h2";
const allowedPackages = new Set(["react-router", "react-router-dom"]);
const command = process.platform === "win32" ? (process.env.ComSpec ?? "cmd.exe") : "npm";
const args = process.platform === "win32"
  ? ["/d", "/s", "/c", "npm audit --omit=dev --json"]
  : ["audit", "--omit=dev", "--json"];
const audit = spawnSync(command, args, {
  cwd: new URL("..", import.meta.url),
  encoding: "utf8",
});

if (audit.error) throw audit.error;
if (!audit.stdout.trim()) {
  throw new Error(`npm audit 未返回 JSON：${audit.stderr.trim() || `退出码 ${audit.status}`}`);
}

let report;
try {
  report = JSON.parse(audit.stdout);
} catch {
  throw new Error(`无法解析 npm audit JSON：${audit.stdout.slice(0, 500)}`);
}

const vulnerabilities = Object.entries(report.vulnerabilities ?? {});
const unexpected = vulnerabilities.filter(([name, finding]) => {
  if (!allowedPackages.has(name)) return true;
  const advisories = finding.via.filter((item) => typeof item === "object");
  return advisories.some((item) => item.url !== allowedAdvisory);
});

if (unexpected.length > 0) {
  const summary = unexpected.map(([name, finding]) => `${name} (${finding.severity})`).join(", ");
  throw new Error(`发现未获例外批准的生产依赖漏洞：${summary}`);
}

const routerFinding = report.vulnerabilities?.["react-router"];
const domFinding = report.vulnerabilities?.["react-router-dom"];
if (!routerFinding || !domFinding) {
  if (vulnerabilities.length === 0 && audit.status === 0) {
    console.log("生产依赖审计通过：没有已知漏洞");
    process.exit(0);
  }
  throw new Error("npm audit 结果不符合已记录的 React Router 临时例外");
}

const routerAdvisories = routerFinding.via.filter((item) => typeof item === "object");
if (routerAdvisories.length !== 1 || routerAdvisories[0].url !== allowedAdvisory) {
  throw new Error("React Router 临时例外的公告集合发生变化，必须重新评估");
}
if (audit.status !== 1) {
  throw new Error(`npm audit 在存在已知例外时返回了意外退出码 ${audit.status}`);
}

console.log(`生产依赖审计通过：仅命中已记录且当前不可利用的 ${allowedAdvisory}`);
