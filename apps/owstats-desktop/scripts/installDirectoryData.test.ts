import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

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
assert.match(hooks, /RMDir \/r "\$INSTDIR\\data"/);
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
assert.doesNotMatch(hooks, /MessageBox[^\r\n]*(?:\$INSTDIR|\$APPDATA)/);

const preUninstall = hooks.indexOf("!macro NSIS_HOOK_PREUNINSTALL");
const postUninstall = hooks.indexOf("!macro NSIS_HOOK_POSTUNINSTALL");
const deleteData = hooks.indexOf('RMDir /r "$INSTDIR\\data"');
assert.ok(preUninstall >= 0);
assert.ok(postUninstall > preUninstall);
assert.ok(
  deleteData > postUninstall,
  "data deletion must run after Tauri's application-running check",
);

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
