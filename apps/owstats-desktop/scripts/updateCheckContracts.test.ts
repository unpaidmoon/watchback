import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const read = (path: string) => readFileSync(resolve(process.cwd(), path), "utf8");
const matches = assert.match;
const rust = read("src-tauri/src/update_check.rs");
const commands = read("src-tauri/src/commands.rs");
const lib = read("src-tauri/src/lib.rs");
const api = read("src/shared/api.ts");
const app = read("src/App.tsx");
const dialog = read("src/features/update/UpdateAnnouncementDialog.tsx");
const csp = (JSON.parse(read("src-tauri/tauri.conf.json")) as { app: { security: { csp: string } } }).app.security.csp;
const readme = read("../../README.md");
const packageJson = JSON.parse(read("package.json")) as { version: string };
const packageLock = JSON.parse(read("package-lock.json")) as {
  version: string;
  packages: Record<string, { version?: string }>;
};
const tauriConfig = JSON.parse(read("src-tauri/tauri.conf.json")) as { version: string };
const cargoManifest = read("src-tauri/Cargo.toml");
const cargoLock = read("src-tauri/Cargo.lock");
const onboarding = read("src/pages/OnboardingPage.tsx");
const releaseManifest = JSON.parse(read("../../deploy/watchback-update/latest.json")) as {
  schemaVersion: number;
  enabled: boolean;
  latestVersion: string;
  title: string;
  notes: string[];
};

matches(rust, /https:\/\/watchback-updates-cn-202608\.oss-cn-chengdu\.aliyuncs\.com\/latest\.json/);
matches(rust, /https_only\(/);
matches(rust, /redirect\(reqwest::redirect::Policy::none\(\)\)/);
matches(rust, /https:\/\/github\.com\/fengpianpianne\/watchback\/releases/);
matches(rust, /https:\/\/pan\.baidu\.com\/s\/5GbzDPA8NkteOswgwKHN5uw/);
matches(rust, /https:\/\/pan\.quark\.cn\/s\/0d7bae24fca3/);
assert.doesNotMatch(dialog, /https?:\/\//, "the frontend dialog must never receive or embed download URLs");
assert.doesNotMatch(api, /openArbitraryUrl|open_url|url:\s*string/i);
matches(api, /openUpdateDownload\s*=\s*\(source: UpdateDownloadSource\)/);
matches(app, /<UpdateAnnouncementDialog/);
for (const command of ["check_app_update", "get_app_version", "open_update_download"]) {
  matches(commands, new RegExp(`fn ${command}\\b`));
  matches(lib, new RegExp(`commands::${command}\\b`));
}
assert.doesNotMatch(csp, /aliyuncs|github|baidu|quark/, "Rust performs update networking; frontend CSP must stay closed");
matches(readme, /程序启动时.*HTTPS.*更新公告/);
matches(readme, /IP/);
matches(readme, /不会发送.*BattleTag/);

const cargoManifestVersion = cargoManifest.match(/^\[package\][\s\S]*?^version = "([^"]+)"/m)?.[1];
const cargoLockVersion = cargoLock.match(/^name = "owstats-desktop"\r?\nversion = "([^"]+)"/m)?.[1];
const readmeVersion = readme.match(/当前版本：`([^`]+)`/)?.[1];
const expectedVersion = packageJson.version;
assert.equal(packageLock.version, expectedVersion, "package-lock top-level version");
assert.equal(packageLock.packages[""]?.version, expectedVersion, "package-lock root package version");
assert.equal(tauriConfig.version, expectedVersion, "tauri.conf version");
assert.equal(cargoManifestVersion, expectedVersion, "Cargo.toml package version");
assert.equal(cargoLockVersion, expectedVersion, "Cargo.lock root package version");
assert.equal(readmeVersion, expectedVersion, "README current version");
assert.doesNotMatch(onboarding, /APP_VERSION|const\s+\w*VERSION\s*=\s*["']\d/);
matches(onboarding, /getAppVersion\(\)/);
matches(api, /invoke<string>\("get_app_version"\)/);
matches(commands, /env!\("CARGO_PKG_VERSION"\)/);
assert.equal(releaseManifest.schemaVersion, 1);
assert.equal(releaseManifest.enabled, false, "the checked-in first-release manifest must stay disabled until packages are published");
assert.equal(releaseManifest.latestVersion, expectedVersion);
assert.ok(releaseManifest.title.length > 0);
assert.ok(releaseManifest.notes.length > 0);

console.log(`update check contract and version consistency tests passed (${expectedVersion})`);
