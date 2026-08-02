import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { detectRouterSecurityViolations } from "./routerSecurityPolicyDetector.mjs";

const scriptsRoot = path.dirname(fileURLToPath(import.meta.url));
const policy = JSON.parse(await readFile(path.join(scriptsRoot, "dependencySecurityPolicy.json"), "utf8"));
const boundary = policy.reactRouterBoundary;

const rejected = [
  'import { RSCStaticRouter } from "react-router/rsc";',
  'export { routeRSCServerRequest } from "react-router/rsc/server";',
  'import "react-router/dom/server";',
  'const server = await import("react-router-dom/server");',
  'const rsc = require("react-router/rsc");',
  "const handler = decodeAction;",
  "const state = decodeFormState(payload);",
  "const entry = entry.rsc;",
];

for (const source of rejected) {
  assert.notEqual(
    detectRouterSecurityViolations(source, boundary).length,
    0,
    `detector should reject: ${source}`,
  );
}

const allowed = [
  'import { HashRouter, Route, Routes } from "react-router-dom";',
  'export { NavLink } from "react-router-dom";',
  'const page = await import("./routes/MatchesPage");',
  'const config = require("./local-config.cjs");',
  'const message = "decode action response";',
];

for (const source of allowed) {
  assert.deepEqual(
    detectRouterSecurityViolations(source, boundary),
    [],
    `detector should allow: ${source}`,
  );
}

console.log("React Router security detector tests passed");
