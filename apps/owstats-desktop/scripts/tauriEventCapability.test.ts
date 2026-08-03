import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const capabilityUrl = new URL("../src-tauri/capabilities/main.json", import.meta.url);
const capability = JSON.parse(await readFile(capabilityUrl, "utf8")) as {
  windows?: string[];
  permissions?: string[];
};

assert.deepEqual(capability.windows, ["main"]);
assert.deepEqual(
  [...(capability.permissions ?? [])].sort(),
  ["core:event:allow-listen", "core:event:allow-unlisten"],
);

console.log("tauri event capability contract passed");
