import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { filtersFromParams, paramsFromFilters, validateFilters } from "../src/features/matches/matchFilters.ts";
import { matchFiltersKey } from "../src/shared/matchesPageCache.ts";

function stripCssComments(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, " ");
}

function hasCssSelectorRule(css: string, selector: string): boolean {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`(?:^|[{},;])\\s*${escapedSelector}\\s*(?=,|\\{)`, "m").test(css);
}

assert.equal(
  hasCssSelectorRule(stripCssComments("/* .matchesPage { min-width: 0; } */"), ".matchesPage"),
  false,
  "commented selector text is not a CSS rule",
);
assert.equal(
  hasCssSelectorRule(stripCssComments(".matchesPage {}"), ".matchesPage"),
  true,
  "a real selector block is a CSS rule",
);

const roundTrip = filtersFromParams(new URLSearchParams("season=22&hero=ana&startDate=2026-07-01&endDate=2026-07-31"));
assert.deepEqual(roundTrip, { season: 22, hero: "ana", dateFrom: "2026-07-01", dateTo: "2026-07-31" });
assert.equal(paramsFromFilters(roundTrip, "match-1").toString(), "matchId=match-1&season=22&hero=ana&startDate=2026-07-01&endDate=2026-07-31");
assert.equal(matchFiltersKey({ hero: "ana", season: 22 }), matchFiltersKey({ season: 22, hero: "ana" }));
assert.deepEqual(
  validateFilters(
    { season: 22, hero: "missing", primaryTag: "invalid" },
    { seasons: [22], heroes: [], maps: [], results: [], modes: [] },
  ),
  { season: 22 },
);

const root = fileURLToPath(new URL("../src/", import.meta.url));
const styleEntry = readFileSync(join(root, "styles.css"), "utf8");
const expectedStyleModules = [
  "./styles/tokens.css",
  "./styles/base.css",
  "./styles/shell.css",
  "./styles/shared.css",
  "./styles/features/matches.css",
  "./styles/features/command-center.css",
  "./styles/features/review.css",
  "./styles/features/ranking.css",
  "./styles/features/friends.css",
] as const;
for (const modulePath of expectedStyleModules) {
  assert.match(styleEntry, new RegExp(`@import "${modulePath}"`));
}
const styleImports = Array.from(
  stripCssComments(styleEntry).matchAll(/^\s*@import\s+(?:url\(\s*)?["']([^"']+)["']\s*\)?[^;\r\n]*;/gm),
  (match) => match[1],
);
const sharedStyleIndex = styleImports.indexOf("./styles/shared.css");
const matchesStyleIndex = styleImports.indexOf("./styles/features/matches.css");
assert.ok(
  sharedStyleIndex >= 0 && sharedStyleIndex < matchesStyleIndex,
  "shared.css is imported before features/matches.css",
);
const localStyleModules = styleImports
  .filter((modulePath) => !modulePath.startsWith("//") && !/^[a-z][a-z\d+.-]*:/i.test(modulePath))
  .map((modulePath) => {
    const resolvedPath = resolve(root, modulePath);
    const pathFromRoot = relative(root, resolvedPath);
    assert.ok(
      pathFromRoot !== "" &&
        pathFromRoot !== ".." &&
        !pathFromRoot.startsWith(`..${sep}`) &&
        !isAbsolute(pathFromRoot),
      `${modulePath} resolves inside src`,
    );
    return { modulePath, css: stripCssComments(readFileSync(resolvedPath, "utf8")) };
  });
const css = [
  "shared.css",
  "tokens.css",
  "base.css",
  "shell.css",
  "features/matches.css",
  "features/command-center.css",
  "features/review.css",
  "features/ranking.css",
  "features/friends.css",
].map((file) => readFileSync(join(root, "styles", file), "utf8")).join("\n");
const matchesStyleModule = localStyleModules.find(
  ({ modulePath }) => modulePath === "./styles/features/matches.css",
);
assert.ok(matchesStyleModule, "features/matches.css resolves to a local CSS module");
const otherLocalStyleModules = localStyleModules.filter(
  ({ modulePath }) => modulePath !== "./styles/features/matches.css",
);
for (const selector of [
  ".matchesPage",
  ".matchesWorkbench",
  ".matchListPane",
  ".matchListCard",
  ".matchDetailPane",
  ".matchTeamTable",
]) {
  assert.ok(hasCssSelectorRule(matchesStyleModule.css, selector), `${selector} is owned by matches.css`);
  const duplicateOwner = otherLocalStyleModules.find(({ css: moduleCss }) =>
    hasCssSelectorRule(moduleCss, selector),
  )?.modulePath;
  assert.equal(duplicateOwner, undefined, `${selector} has no rule outside matches.css`);
}
for (const selector of [".phase6ProductShell", ".phase6MatchWorkbench", ".phase6CommandCenter", ".phase6RankingTable", ".phase6Friend", ".phase6AchievementWall", ".phase7ArenaSelector"]) {
  assert.ok(css.includes(selector), `${selector} should remain available across style modules`);
}
