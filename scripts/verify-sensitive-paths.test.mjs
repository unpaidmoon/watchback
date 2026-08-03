import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { scanRoot } from "./verify-sensitive-paths.mjs";

const fixtures = new URL("./fixtures/verify/", import.meta.url);
const fixturePath = (name) => fileURLToPath(new URL(`${name}/`, fixtures));
assert.deepEqual(scanRoot(fixturePath("clean")), []);
assert.deepEqual(scanRoot(fixturePath("allowed-token-style")), []);
const absolute = scanRoot(fixturePath("absolute-path"));
assert.equal(absolute.length, 1);
assert.equal(absolute[0].rule, "absolute-path");
const sensitive = scanRoot(fixturePath("sensitive-key"));
assert.equal(sensitive.length, 1);
assert.equal(sensitive[0].rule, "sensitive-key");
assert.equal(JSON.stringify(sensitive).includes("synthetic-secret"), false);
const accountKey = scanRoot(fixturePath("account-key"));
assert.equal(accountKey.length, 1);
assert.equal(accountKey[0].rule, "sensitive-key");
