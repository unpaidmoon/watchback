import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const config = JSON.parse(
  readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);
assert.equal(config.bundle.targets, "nsis");
assert.equal(config.bundle.windows.nsis.installMode, "both");
assert.deepEqual(config.bundle.windows.nsis.languages, ["SimpChinese"]);
assert.equal(
  config.bundle.windows.nsis.installerHooks,
  "./windows/installer-hooks.nsh",
);
assert.equal(config.bundle.windows.nsis.template, "./windows/installer.nsi");

const hooks = readFileSync(
  new URL("../src-tauri/windows/installer-hooks.nsh", import.meta.url),
  "utf8",
);
for (const macro of [
  "NSIS_HOOK_PREINSTALL",
  "NSIS_HOOK_POSTINSTALL",
  "NSIS_HOOK_PREUNINSTALL",
  "NSIS_HOOK_POSTUNINSTALL",
]) {
  assert.match(hooks, new RegExp(`!macro ${macro}`));
}

assert.match(hooks, /\$\{MAINBINARYNAME\}\.exe/);
assert.match(hooks, /--prepare-install-data/);
assert.match(hooks, /InstallLocation/);
assert.match(hooks, /Software\\\$\{BUNDLEID\}/);
assert.match(hooks, /nsExec::ExecToStack/);
assert.match(hooks, /Pop \$[0-9R]/);
assert.match(hooks, /GetOwnerSid/);
assert.match(hooks, /explorer\.exe/i);
assert.match(hooks, /ProfileList/);
assert.match(hooks, /User Shell Folders/);
assert.match(hooks, /S-1-5-18/);
assert.match(hooks, /\/grant:r\s+"\*\$/i);
assert.match(hooks, /nsis_tauri_utils::RunAsUser/);
assert.match(hooks, /WATCHBACK_DATA_PROBE/);
assert.match(hooks, /\$INSTDIR\\data/);
assert.match(hooks, /Delete "\$INSTDIR\\data\\\.write-test-\$/);
assert.match(hooks, /\/SD IDNO/);
assert.match(hooks, /--delete-install-data "\$INSTDIR\\data"/);
assert.match(
  hooks,
  /--delete-install-data[^]*?Pop \$1\s+Pop \$2\s+StrCpy \$2 ""\s+\$\{If\} \$1 != 0[^]*?!insertmacro InstallDataAbort/,
);
assert.match(hooks, /本地数据将保留在原安装目录/);
assert.match(hooks, /该目录可能不会被完全移除/);
assert.match(hooks, /StrCpy \$DeleteAppDataCheckboxState 0/);
assert.match(hooks, /GetFullPathName/);
assert.match(
  hooks,
  /ReadRegStr \$2 SHCTX "Software\\\$\{BUNDLEID\}" "InstallLocation"\s+GetFullPathName \$2 "\$2"/,
);
assert.match(hooks, /\$\{OrIf\} \$2 != "\$0"/);

assert.doesNotMatch(hooks, /\$USERNAME/);
assert.doesNotMatch(hooks, /Builtin\\Users|BUILTIN\\Users|S-1-5-32-545/i);
assert.doesNotMatch(hooks, /grant[^\r\n]*"\$INSTDIR"(?:\s|$)/i);
assert.doesNotMatch(hooks, /RMDir \/r "\$INSTDIR"(?:\s|$)/);
assert.doesNotMatch(hooks, /RMDir \/r "\$INSTDIR\\data"/);
assert.doesNotMatch(hooks, /MessageBox[^\r\n]*(?:\$INSTDIR|\$APPDATA)/);

const preUninstall = hooks.indexOf("!macro NSIS_HOOK_PREUNINSTALL");
const postUninstall = hooks.indexOf("!macro NSIS_HOOK_POSTUNINSTALL");
const deleteData = hooks.indexOf("--delete-install-data");
assert.ok(preUninstall >= 0);
assert.ok(postUninstall > preUninstall);
assert.ok(
  deleteData > postUninstall,
  "data deletion must run after Tauri's application-running check",
);

const templateUrl = new URL(
  "../src-tauri/windows/installer.nsi",
  import.meta.url,
);
const templateBytes = readFileSync(templateUrl);
const template = templateBytes.toString("utf8");
assert.equal(
  createHash("sha256").update(template.replace(/\r\n/g, "\n")).digest("hex"),
  "934153506da4954298a76285e889fe71ca89f23b207e9db4679d488542ae5e62",
);
assert.match(template, /Tauri CLI 2\.11\.3 tag tauri-cli-v2\.11\.3/);
assert.match(template, /d372e3c391770cf231db974422a1e4f8adaac3a6/);
assert.match(template, /Function PageLeaveReinstall/);
assert.match(template, /WATCHBACK-UPGRADE-PARENT=\$R9/);
assert.match(template, /GetCurrentProcessId/);
assert.match(template, /StrCpy \$WatchbackNestedUpgrade 1/);
assert.doesNotMatch(
  template,
  /-Command "[^"\r\n]*\$R8/,
  "untrusted parent PID must never be interpolated into PowerShell source",
);
assert.doesNotMatch(template, /-Command "[^"\r\n]*WATCHBACK-UPGRADE/);
assert.match(template, /-EncodedCommand \$\{WATCHBACK_UPGRADE_PARENT_PROOF\}/);
assert.match(
  template,
  /SetEnvironmentVariableW\(w "WATCHBACK_UPGRADE_PARENT_PID", w "\$R8"\)/,
);
const encodedParentProof = template.match(
  /!define WATCHBACK_UPGRADE_PARENT_PROOF "([A-Za-z0-9+/=]+)"/,
)?.[1];
assert.ok(encodedParentProof);
const parentProof = Buffer.from(encodedParentProof, "base64").toString(
  "utf16le",
);
assert.match(parentProof, /WATCHBACK_UPGRADE_PARENT_PID/);
assert.match(parentProof, /WATCHBACK_UNINSTALLER_PID/);
assert.match(parentProof, /\[uint32\]::TryParse/);
assert.match(parentProof, /\^\[0-9\]\{1,10\}\$/);
assert.match(parentProof, /ParentProcessId-ne\$p/);
assert.match(parentProof, /VersionInfo\.ProductName-ne'Watchback'/);
assert.doesNotMatch(parentProof, /Invoke-Expression|iex|ScriptBlock/i);

const canonicalUint32 = (value: string): boolean =>
  /^[0-9]{1,10}$/.test(value) &&
  BigInt(value) <= 4_294_967_295n &&
  BigInt(value).toString() === value;
for (const invalid of [
  "",
  "+1",
  "(1)",
  "$(Write-Output injected)",
  "1;Write-Output injected",
  "1#comment",
  " 1",
  "1 ",
  "-1",
  "4294967296",
  "01",
]) {
  assert.equal(canonicalUint32(invalid), false, invalid);
}
for (const valid of ["1", "4294967295"]) {
  assert.equal(canonicalUint32(valid), true, valid);
}

if (process.platform === "win32") {
  const proofTemp = mkdtempSync(join(tmpdir(), "watchback-parent-proof-"));
  try {
    for (const [index, invalid] of [
      "",
      "+1",
      "(1)",
      "$(Write-Output injected)",
      "1;[IO.File]::WriteAllText($env:WATCHBACK_PROOF_SENTINEL,'pwned')",
      "1#comment",
      " 1",
      "1 ",
      "-1",
      "4294967296",
      "01",
    ].entries()) {
      const sentinel = join(proofTemp, `sentinel-${index}`);
      const result = spawnSync(
        "powershell.exe",
        [
          "-NoLogo",
          "-NoProfile",
          "-NonInteractive",
          "-EncodedCommand",
          encodedParentProof,
        ],
        {
          env: {
            ...process.env,
            WATCHBACK_UPGRADE_PARENT_PID: invalid,
            WATCHBACK_UNINSTALLER_PID: process.pid.toString(),
            WATCHBACK_PROOF_SENTINEL: sentinel,
          },
          encoding: "utf8",
        },
      );
      assert.equal(result.status, 1, invalid);
      assert.equal(existsSync(sentinel), false, invalid);
    }
  } finally {
    rmSync(proofTemp, { recursive: true, force: true });
  }
}
assert.match(
  template,
  /\$\{Else\}\s+; A claimed nested setup must not fall back to trusting \/UPDATE alone\.\s+StrCpy \$UpdateMode 0/,
);
const preInstallHook = template.indexOf("!insertmacro NSIS_HOOK_PREINSTALL");
const runningCheck = template.indexOf(
  '!insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"',
);
const preUninstallHook = template.indexOf("!insertmacro NSIS_HOOK_PREUNINSTALL");
const uninstallRunningCheck = template.indexOf(
  '!insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"',
  runningCheck + 1,
);
const postUninstallHook = template.indexOf("!insertmacro NSIS_HOOK_POSTUNINSTALL");
const delayedExeDelete = template.indexOf(
  'Delete "$INSTDIR\\${MAINBINARYNAME}.exe"',
  postUninstallHook,
);
assert.ok(preInstallHook >= 0 && runningCheck > preInstallHook);
assert.ok(preUninstallHook >= 0 && uninstallRunningCheck > preUninstallHook);
assert.ok(postUninstallHook > uninstallRunningCheck);
assert.ok(delayedExeDelete > postUninstallHook);

const oldInstallSource = hooks.indexOf(
  'StrCpy $0 "$InstallDataOldLocation\\data"',
);
const existingDestination = hooks.indexOf(
  '${FileExists} "$INSTDIR\\data\\*.*"',
);
const legacyAppDataSource = hooks.indexOf(
  'StrCpy $0 "$InstallDataLegacyAppData\\local.owstats.desktop"',
);
assert.ok(oldInstallSource >= 0);
assert.ok(existingDestination > oldInstallSource);
assert.ok(legacyAppDataSource > existingDestination);

const encodedProbe = hooks.match(
  /!define INSTALL_DATA_PROBE_COMMAND "([A-Za-z0-9+/=]+)"/,
)?.[1];
assert.ok(encodedProbe);
const probe = Buffer.from(encodedProbe, "base64").toString("utf16le");
assert.match(probe, /WATCHBACK_DATA_PROBE/);
assert.match(probe, /WriteAllText/);
assert.doesNotMatch(probe, /Users|Everyone|S-1-5-32-545/i);

const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
assert.match(
  packageJson.scripts["test:resources"],
  /tsx scripts\/installDirectoryData\.test\.ts$/,
);
