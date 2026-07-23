import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { arenaHeaderMapKey, arenaHeaderTitle, formatArenaRankTierLabel, formatArenaScore, formatArenaTabLabel } from "../src/shared/arenaDetailFormat";
import type { ArenaDetailSlice } from "../src/shared/types";

const summary = slice({ key: "summary", label: "角斗汇总", teamScore: 4, opponentScore: 3, mapGuid: "kings-row", mapName: "国王大道" });
const round = slice({ key: "round-4", label: "回合 4", teamScore: 2, opponentScore: 2, mapName: "阿努比斯王座" });

assert.equal(formatArenaScore(summary), "4 - 3");
assert.equal(formatArenaScore(round), "2 - 2");
assert.equal(formatArenaTabLabel(round), "回合-4");
assert.equal(arenaHeaderMapKey(summary), "kings-row");
assert.equal(formatArenaRankTierLabel("大师 2"), "2");
assert.equal(formatArenaRankTierLabel("钻石 5"), "5");
assert.equal(formatArenaRankTierLabel("大师"), "");
assert.equal(arenaHeaderTitle(summary), "国王大道");
assert.equal(arenaHeaderTitle(round), "地图：阿努比斯王座");

const styleSource = readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8");
const detailSource = readFileSync(new URL("../src/pages/MatchDetailPage.tsx", import.meta.url), "utf8");
const compactSource = readFileSync(new URL("../src/features/matches/CompactMatchDetail.tsx", import.meta.url), "utf8");
const teamTableSource = readFileSync(new URL("../src/features/matches/MatchTeamTable.tsx", import.meta.url), "utf8");

assert.equal(styleSource.includes(".phase7ArenaTab + .phase7ArenaTab"), true, "arena tabs should overlap adjacent slants");
assert.equal(styleSource.includes("gap: 0"), true, "arena selector should not leave gaps between tabs");
assert.equal(detailSource.includes("phase7PlayerNameLine"), true, "rank should sit in the player id line");
assert.match(
  styleSource,
  /\.phase7CompactArenaDetail\s*\{[^}]*grid-template-rows:\s*auto\s+124px\s+minmax\(0,\s*1fr\)/s,
  "compact arena detail should reserve separate rows for tabs, hero banner, and scroll content",
);
assert.equal(teamTableSource.includes("phase7CompactPlayerNameLine"), true, "compact rank should sit in the player id line");
assert.equal(teamTableSource.includes("playerHighlightMap"), true, "compact arena rows should use source max highlights");
assert.equal(compactSource.includes("const [expandedCompactArenaPlayerKeys"), true, "compact arena detail expansion state should live in MatchesPage");
assert.equal(compactSource.includes("setExpandedCompactArenaPlayerKeys(new Set())"), true, "compact arena expansion state should reset when match or slice changes");
assert.equal(compactSource.includes("function selectCompactArenaSlice"), true, "compact arena slice changes should synchronously clear expansion state");
assert.equal(compactSource.includes("function toggleCompactArenaPlayer"), true, "compact arena rows should have a local expansion toggle");
assert.equal(detailSource.includes("const [expandedArenaPlayerKeys"), false, "full detail should not own arena expansion state for this compact-only slice");
assert.equal(detailSource.includes('allowExpansion={true}'), false, "full arena detail should not enable expansion in this compact-only slice");
assert.equal(detailSource.includes("const [expanded, setExpanded] = useState(false)"), false, "player rows should not keep isolated local expansion state");
assert.equal(compactSource.includes("phase7CompactExpandButton"), false, "compact detail should not show a separate expansion button");
assert.equal(teamTableSource.includes("phase7CompactPlayerExpandable"), true, "compact arena player rows should be directly clickable for expansion");
assert.equal(teamTableSource.includes("onClick={onToggle}"), true, "compact arena player rows should toggle expansion from the row itself");
assert.equal(teamTableSource.includes("aria-expanded={expanded}"), true, "clickable compact arena rows should expose expanded state");
assert.equal(detailSource.includes('className="phase7ExpandButton"'), false, "full detail should not keep the mistaken expansion button");
assert.match(styleSource, /\.phase7PlayerCardMain\s*\{[^}]*grid-template-columns:\s*168px\s+minmax\(0,\s*1fr\)/s, "expanded arena rows should reserve stable portrait and content columns");
assert.match(styleSource, /\.phase6PlayerStats header\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto/s, "expand control should stay in a dedicated header column");
assert.match(styleSource, /\.phase7PlayerNameLine\s*\{[^}]*flex-wrap:\s*wrap/s, "arena player name and rank should wrap before crowding metrics");
assert.match(styleSource, /\.phase6CompactPlayerRow\.phase7CompactPlayerExpandable\s*\{[^}]*cursor:\s*pointer/s, "compact arena player rows should show direct click affordance");
assert.match(styleSource, /\.phase6CompactPlayerRow\.phase7CompactPlayerExpandable:focus-visible\s*\{[^}]*outline:/s, "clickable compact arena rows should have keyboard focus styling");
assert.equal(compactSource.includes('className="phase7CompactArenaMoneyIcon"'), true, "compact arena economy should use a dedicated money icon area");
assert.equal(compactSource.includes('className="phase7CompactArenaMoneyLabel"'), true, "compact arena economy should label money in Chinese");
assert.equal(compactSource.includes("const variant = slots === 4 ? \"trait\" : \"mod\""), true, "compact arena slot groups should derive trait and equipment visual modes from fixed slot counts");
assert.equal(compactSource.includes("phase7ArenaSlotName"), true, "compact arena trait slots should render mapped names beside icons");
assert.equal(compactSource.includes("const displayName = info?.name ?? guid"), true, "unmapped arena slots should use their raw GUID as the display fallback");
assert.equal(compactSource.includes("fallbackName="), false, "arena slot groups should not keep a pending-text fallback");
assert.equal(compactSource.includes('CompactArenaSlotGroup title="异能" guids={player.traitGuids} slots={4} />'), true, "compact arena trait slots should be fixed to four without a pending-text fallback");
assert.equal(compactSource.includes('CompactArenaSlotGroup title="装备" guids={player.modGuids} slots={6} />'), true, "compact arena mod slots should be fixed to six without a pending-text fallback");
assert.equal(compactSource.includes("aria-label={guid ? displayName : undefined}"), true, "missing compact arena slot mappings should expose their raw GUID in the accessibility label");
assert.equal(compactSource.includes("<strong>{displayName}</strong>"), true, "missing compact arena slot mappings should expose their raw GUID in the tooltip");
assert.equal(compactSource.includes("暂无装备"), false, "empty compact arena mod slots should not render empty copy");
assert.equal(compactSource.includes("暂无异能"), false, "empty compact arena trait slots should not render empty copy");
assert.match(styleSource, /\.phase7CompactArenaExpansion\s*\{[^}]*grid-template-columns:\s*minmax\(110px,\s*0\.55fr\)\s+minmax\(180px,\s*1fr\)\s+minmax\(240px,\s*1\.25fr\)/s, "compact arena expansion should organize economy, traits, and equipment horizontally when space allows");
assert.match(styleSource, /@media\s*\(max-width:\s*1080px\)\s*\{[\s\S]*\.phase7CompactArenaExpansion\s*\{[^}]*grid-template-columns:\s*1fr/s, "narrow compact arena expansion should stack economy, traits, and equipment vertically");
assert.match(styleSource, /\.phase7CompactArenaExpansion\s*\{[^}]*grid-template-columns:\s*150px\s+minmax\(300px,\s*1fr\)\s+minmax\(260px,\s*0\.82fr\)/s, "compact arena expansion should organize economy, traits, and equipment like the expected dark panel");
assert.match(styleSource, /\.phase7ArenaSlotGroup\.trait \.phase7ArenaSlots\s*\{[^}]*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\)/s, "compact arena traits should render as a two-by-two icon and name grid");
assert.match(styleSource, /\.phase7ArenaSlotGroup\.mod \.phase7ArenaSlots\s*\{[^}]*grid-template-columns:\s*repeat\(3,\s*48px\)/s, "compact arena equipment should render as a three-by-two round icon grid");
assert.match(styleSource, /\.phase7ArenaSlot\.mod\s*\{[^}]*border-radius:\s*999px/s, "compact arena equipment slots should be round");
assert.doesNotMatch(styleSource, /\.phase7ArenaSlot\.mod\s*\{[^}]*background:\s*#[7-9a-fA-F][0-9a-fA-F]{5}/s, "compact arena equipment slots should not force all icons to a purple color");
assert.match(styleSource, /\.phase7ArenaSlotGroup\s*\{[^}]*overflow:\s*visible/s, "arena slot groups should not clip slot tooltips");
assert.match(styleSource, /\.phase7ArenaSlot\s*\{[^}]*overflow:\s*visible/s, "arena slots should not clip tooltip panels");
assert.match(styleSource, /\.phase7ArenaSlot \.phase6PerkTooltipPanel\s*\{[^}]*z-index:\s*80/s, "arena slot tooltips should rise above neighboring cards");
assert.match(compactSource, /className=\{player\.worthMax \? "phase6StatHighlight" : ""\}/, "worthMax should highlight the compact economy value");
assert.match(
  compactSource,
  /if \(mode === "round"\) \{[\s\S]*if \(player\.endWorth !== null\) return String\(player\.endWorth\);[\s\S]*if \(player\.worth !== null\) return String\(player\.worth\);/,
  "compact arena round economy should show cumulative end worth without a transition",
);
assert.doesNotMatch(compactSource, /return `\$\{player\.worth\} -> \$\{player\.endWorth\}`;/);
assert.equal(compactSource.includes('economyMode={selected.key === "summary" ? "final" : "round"}'), true, "compact arena summary and rounds should use separate economy display modes");
assert.equal(compactSource.includes('function formatCompactArenaEconomy(player: PlayerRow, mode: "final" | "round"): string'), true, "compact arena economy formatter should know whether it is rendering summary or round detail");
const highlightRule = styleSource.match(/\.phase6StatHighlight\s*\{(?<body>[^}]*)\}/)?.groups?.body ?? "";
assert.equal(highlightRule.includes("background:"), false, "stat highlight should use slight text color only, without a background block");
assert.equal(highlightRule.includes("text-shadow:"), false, "stat highlight should avoid glow");
assert.equal(highlightRule.includes("padding:"), false, "stat highlight should not change metric spacing");
assert.match(styleSource, /\.phase7ArenaRankBadge,\s*\.phase7CompactRankBadge\s*\{[^}]*color:\s*#fff/s, "arena rank tier text should be white");
assert.match(
  styleSource,
  /\.phase6PlayerStats header \.phase7ArenaRankBadge,\s*\.phase6CompactPlayerRow \.phase7CompactRankBadge\s*\{[^}]*color:\s*#fff/s,
  "arena rank tier text should stay white against parent small/span color rules",
);
assert.match(styleSource, /\.phase7ArenaRankBadge img,\s*\.phase7CompactRankBadge img\s*\{[^}]*width:\s*24px[^}]*height:\s*24px/s, "arena rank icons should be larger");

function slice(overrides: Partial<ArenaDetailSlice>): ArenaDetailSlice {
  return {
    key: "summary",
    label: "角斗汇总",
    result: "win",
    resultLabel: "胜利",
    teamScore: null,
    opponentScore: null,
    durationSec: null,
    beginTs: null,
    beginLabel: null,
    mapGuid: null,
    mapName: null,
    teammates: [],
    enemies: [],
    ...overrides,
  };
}
