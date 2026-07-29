import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const source = readFileSync(
  new URL("../src/features/command-center/QrAccountLoginDialog.tsx", import.meta.url),
  "utf8",
);

const directStart = source.indexOf("const { sessionId } = await addAccountWithQr()");
const sessionAssignment = source.indexOf("sessionIdRef.current = sessionId", directStart);
const snapshotPoll = source.indexOf("getQrAccountLoginSnapshot(sessionId)");

assert.ok(
  directStart >= 0 && sessionAssignment > directStart,
  "QR login must save the backend session as soon as the start command resolves",
);
assert.ok(
  snapshotPoll >= 0,
  "QR login must poll its local backend snapshot instead of depending on event registration",
);
assert.match(source, /setMessage\("\\u767b\\u5f55\\u73af\\u5883\\u542f\\u52a8\\u8d85\\u65f6/);
assert.doesNotMatch(
  source,
  /listen<[^>]+>\("credential-login-qr/,
  "QR display must not depend on a Tauri event listener",
);
