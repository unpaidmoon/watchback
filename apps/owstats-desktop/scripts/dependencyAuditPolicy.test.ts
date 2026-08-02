import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { validateAuditReport } from "./dependencyAuditPolicy.mjs";

const policy = JSON.parse(await readFile(new URL("./dependencySecurityPolicy.json", import.meta.url), "utf8"));
const beforeExpiry = Date.parse("2026-08-03T00:00:00.000Z");
const allowedUrl = policy.reactRouterException.allowedAdvisoryUrls[0];
const allowedReport = {
  vulnerabilities: {
    "react-router": { severity: "high", via: [{ url: allowedUrl }] },
    "react-router-dom": { severity: "high", via: ["react-router"] },
  },
};

assert.equal(validateAuditReport({ vulnerabilities: {} }, 0, policy, beforeExpiry).mode, "clean");
assert.equal(validateAuditReport(allowedReport, 1, policy, beforeExpiry).mode, "exception");
assert.throws(
  () => validateAuditReport({
    vulnerabilities: {
      ...allowedReport.vulnerabilities,
      "new-package": { severity: "high", via: [{ url: "https://example.invalid/new" }] },
    },
  }, 1, policy, beforeExpiry),
  /Unapproved vulnerable production package/,
);
assert.throws(
  () => validateAuditReport({
    vulnerabilities: {
      "react-router": { severity: "high", via: [{ url: "https://example.invalid/new-advisory" }] },
      "react-router-dom": { severity: "high", via: ["react-router"] },
    },
  }, 1, policy, beforeExpiry),
  /Advisory set changed/,
);
assert.throws(
  () => validateAuditReport(allowedReport, 1, policy, Date.parse("2026-09-03T00:00:00.000Z")),
  /expired on 2026-09-02/,
);
assert.throws(
  () => validateAuditReport({ vulnerabilities: {} }, 1, policy, beforeExpiry),
  /reported no vulnerabilities but exited with 1/,
);
assert.throws(
  () => validateAuditReport({
    vulnerabilities: {
      "react-router": { severity: "high", via: ["react-router-dom"] },
      "react-router-dom": { severity: "high", via: ["react-router"] },
    },
  }, 1, policy, beforeExpiry),
  /expected advisory/,
);

console.log("Dependency audit policy tests passed");
