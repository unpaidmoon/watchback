import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptsRoot = path.dirname(fileURLToPath(import.meta.url));
const testFiles = readdirSync(scriptsRoot).filter((file) => /\.test\.tsx?$/.test(file));
let assertMatchCount = 0;
const sourceTextTests = testFiles.flatMap((file) => {
  const content = readFileSync(path.join(scriptsRoot, file), "utf8");
  assertMatchCount += (content.match(/assert\.match\b/g) ?? []).length;
  const readsFiles = /\breadFile(?:Sync)?\b/.test(content);
  const referencesProductionSource = /(?:\.\.\/)?src(?:-tauri)?\//.test(content);
  if (!readsFiles || !referencesProductionSource) return [];
  return [{
    file,
    regexAssertions: (content.match(/assert\.(?:doesNot)?match\b/g) ?? []).length,
  }];
});

const sourceRegexAssertions = sourceTextTests.reduce((total, test) => total + test.regexAssertions, 0);
assert.ok(
  sourceTextTests.length <= 10,
  `源码文本型测试 ${sourceTextTests.length} 个，超过预算 10：${sourceTextTests.map(({ file }) => file).join(", ")}`,
);
assert.ok(
  assertMatchCount <= 200,
  `assert.match ${assertMatchCount} 个，超过预算 200`,
);

console.log(
  `测试结构预算通过：${sourceTextTests.length} 个源码文本文件，${sourceRegexAssertions} 个源码正则断言，${assertMatchCount} 个 assert.match`,
);
