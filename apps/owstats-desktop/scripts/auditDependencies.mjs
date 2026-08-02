import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { validateAuditReport, validateSecurityPolicy } from "./dependencyAuditPolicy.mjs";

const policy = JSON.parse(await readFile(new URL("./dependencySecurityPolicy.json", import.meta.url), "utf8"));
const { exception } = validateSecurityPolicy(policy);

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
  throw new Error(`npm audit returned no JSON: ${audit.stderr.trim() || `exit code ${audit.status}`}`);
}

let report;
try {
  report = JSON.parse(audit.stdout);
} catch {
  throw new Error(`Unable to parse npm audit JSON: ${audit.stdout.slice(0, 500)}`);
}

const result = validateAuditReport(report, audit.status, policy);
if (result.mode === "clean") {
  console.log("Production dependency audit passed: no known vulnerabilities");
  process.exit(0);
}

console.log(
  `Production dependency audit passed under exception ${exception.id} (expires ${exception.expiresOn})`,
);
