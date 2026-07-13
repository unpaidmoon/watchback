import assetManifest from "../../../../assets/manifest.json";
import rankManifest from "../../../../assets/ranks/manifest.json";
import stadiumManifest from "../../../../assets/stadium/manifest.json";
import runtimeLookups from "../../runtime-config/ow-lookups.json";

const repoAssetRoot = "/assets";
const rankAssetRoot = `${repoAssetRoot}/ranks`;
const achievementAssetRoot = `${repoAssetRoot}/achievements/token-style`;
const stadiumAssetRoot = `${repoAssetRoot}/stadium`;

type ManifestEntry = {
  key: string;
  name?: string;
  cn_name?: string;
  aliases?: string[];
  source_url?: string;
  local_path?: string;
};

type AssetManifest = {
  heroes: ManifestEntry[];
  maps: ManifestEntry[];
  perks?: ManifestEntry[];
};

type RankManifest = {
  ranks: { key: string; file: string | null }[];
};

type StadiumManifest = {
  fightRanks?: StadiumRankEntry[];
  modTrait?: StadiumModTraitEntry[];
};

type StadiumRankEntry = {
  key: string;
  localPath?: string;
  pureLocalPath?: string;
};

type StadiumModTraitEntry = {
  id?: string;
  decimalId?: string;
  hexId?: string;
  name?: string;
  desc?: string;
  description?: string;
  localPath?: string;
  category?: string;
};

const manifest = assetManifest as AssetManifest;
const ranks = rankManifest as RankManifest;
const stadium = stadiumManifest as StadiumManifest;
const rankFiles = new Map(ranks.ranks.filter((rank) => rank.file).map((rank) => [rank.key, rank.file as string]));
const arenaRankFiles = new Map(
  (stadium.fightRanks ?? []).flatMap((rank) => {
    const key = normalizeAssetKey(rank.key);
    const file = rank.localPath ?? rank.pureLocalPath;
    const url = normalizeStadiumAssetPath(file);
    return key && url ? [[key, url] as const] : [];
  }),
);

type QueryToolPerk = {
  id?: string;
  guid?: string;
  name?: string;
  icon?: string;
  desc?: string;
};

type QueryToolConfig = {
  heroes?: Record<string, { name?: string }>;
  maps?: Record<string, { name?: string }>;
  heroPerkList?: Record<string, QueryToolPerk[]>;
  perkList?: QueryToolPerk[];
  heroAttrList?: QueryToolHeroAttr[];
};

type OwConfigCache = {
  attrs?: Record<string, { text?: string; type?: string }>;
};

export type PerkInfo = {
  id: string;
  guid: string;
  name: string;
  description: string | null;
  iconUrl: string | null;
};

type QueryToolHeroAttr = {
  heroGuid?: string;
  heroName?: string;
  valueGuid?: string;
  valueText?: string;
  valueType?: string;
};

export type HeroStatInfo = {
  guid: string;
  label: string;
  type: string | null;
};

const queryTool = runtimeLookups as QueryToolConfig;
const owConfig = runtimeLookups as OwConfigCache;

const localizedMapAliases: Record<string, string> = {
  阿特利斯: "aatlis",
  南极半岛: "antarctic-peninsula",
  阿努比斯: "anubis",
  阿努比斯王座: "throne-of-anubis",
  暴雪世界: "blizzard-world",
  釜山: "busan",
  皇家赛道: "circuit-royal",
  多拉多: "dorado",
  生态监测站南极洲: "ecopoint-antarctica",
  艾兴瓦尔德: "eichenwalde",
  埃斯佩兰萨: "esperanca",
  花村: "hanamura",
  哈瓦那: "havana",
  好莱坞: "hollywood",
  伊利奥斯: "ilios",
  渣客镇: "junkertown",
  卡内扎卡: "kanezaka",
  国王大道: "kings-row",
  漓江塔: "lijiang-tower",
  米德城: "midtown",
  尼泊尔: "nepal",
  新渣客城: "new-junk-city",
  新皇后街: "new-queen-street",
  努巴尼: "numbani",
  绿洲城: "oasis",
  帕拉伊苏: "paraiso",
  巴黎: "paris",
  佩特拉: "petra",
  里阿尔托: "rialto",
  "66号公路": "route-66",
  鲁纳萨彼: "runasapi",
  萨摩亚: "samoa",
  香巴里寺院: "shambali-monastery",
  苏拉瓦萨: "suravasa",
  泰晤士区: "thames-district",
  沃斯卡娅工业区: "volskaya",
  监测站直布罗陀: "watchpoint-gibraltar",
  五星大学: "wuxing-university",
};

const stadiumMapPreviewFiles = new Map<string, string>([
  ["576460752303428183", "thames-district.jpg"],
  ["576460752303428186", "gogadoro.jpg"],
  ["576460752303428190", "place-lacroix.jpg"],
  ["576460752303428192", "redwood-dam.jpg"],
  ["576460752303428209", "powder-keg-mine.jpg"],
  ["576460752303428219", "ilios2.png"],
  ["576460752303428223", "oasis3.png"],
  ["576460752303428266", "arena-victoriae.jpg"],
  ["576460752303428272", "esperanca2.png"],
  ["576460752303428277", "throne-of-anubis.jpg"],
  ["576460752303428278", "hanaoka.jpg"],
  ["576460752303428279", "colosseo.jpg"],
  ["576460752303428337", "nepal2.png"],
  ["576460752303428437", "samoa2.png"],
  ["576460752303428469", "wuxing-university.jpg"],
  ["576460752303428488", "powder-keg-mine.jpg"],
  ["576460752303428212", "serenza.png"],
  ["576460752303428683", "lijiangtower2.png"],
  ["576460752303429014", "oasis-university.png"],
]);

function manifestLookup(entries: ManifestEntry[]) {
  const lookup = new Map<string, string>();
  for (const entry of entries) {
    for (const value of [entry.key, entry.name, entry.cn_name, ...(entry.aliases ?? [])]) {
      if (!value) continue;
      lookup.set(value.trim().toLowerCase(), entry.key);
      const normalized = normalizeAssetKey(value);
      if (normalized) lookup.set(normalized, entry.key);
    }
  }
  return lookup;
}

function displayNameLookup(entries: ManifestEntry[]) {
  const lookup = new Map<string, string>();
  for (const entry of entries) {
    const displayName = entry.cn_name?.trim() || entry.name?.trim();
    if (!displayName) continue;
    for (const value of [entry.key, entry.name, entry.cn_name, ...(entry.aliases ?? [])]) {
      const raw = value?.trim();
      if (!raw) continue;
      lookup.set(raw.toLowerCase(), displayName);
      const normalized = normalizeAssetKey(raw);
      if (normalized) lookup.set(normalized, displayName);
    }
  }
  return lookup;
}

const heroLookup = manifestLookup(manifest.heroes);
for (const [guid, hero] of Object.entries(queryTool.heroes ?? {})) {
  const heroName = hero.name?.trim();
  const key = heroName ? assetKey(heroName, heroLookup) : null;
  if (key) heroLookup.set(guid, key);
}
const mapLookup = new Map([...manifestLookup(manifest.maps), ...Object.entries(localizedMapAliases).map(([name, key]) => [name.toLowerCase(), key] as const)]);
const heroDisplayNames = displayNameLookup(manifest.heroes);
const mapDisplayNames = displayNameLookup(manifest.maps);
for (const [guid, hero] of Object.entries(queryTool.heroes ?? {})) {
  const name = hero.name?.trim();
  if (name) heroDisplayNames.set(guid, resolveDisplayName(name, name, heroDisplayNames, name));
}
for (const [guid, map] of Object.entries(queryTool.maps ?? {})) {
  const name = map.name?.trim();
  if (name) mapDisplayNames.set(guid, resolveDisplayName(name, name, mapDisplayNames, name));
}
const perkIconLookup = buildPerkIconLookup(manifest.perks ?? []);
const perkLookup = buildPerkLookup(queryTool, perkIconLookup);
const heroStatLookup = buildHeroStatLookup(queryTool, owConfig);
const arenaModTraitLookup = buildArenaModTraitLookup(stadium.modTrait ?? []);

export function normalizeAssetKey(value: string | null | undefined): string | null {
  const normalized = value
    ?.trim()
    .toLowerCase()
    .replace(/['’]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalized ? normalized : null;
}

function resolveDisplayName(
  key: string | null | undefined,
  name: string | null | undefined,
  lookup: Map<string, string>,
  fallback: string,
) {
  for (const value of [key, name]) {
    const raw = value?.trim();
    if (!raw) continue;
    const normalized = normalizeAssetKey(raw);
    const resolved = lookup.get(raw.toLowerCase()) ?? (normalized ? lookup.get(normalized) : undefined);
    if (resolved) return resolved;
  }
  const persisted = name?.trim();
  if (persisted && persisted !== key && !/^\d{12,}$/.test(persisted) && !/^0x[0-9a-f]+$/i.test(persisted)) return persisted;
  return fallback;
}

export function resolveHeroDisplayName(heroKey: string | null | undefined, name?: string | null): string {
  return resolveDisplayName(heroKey, name, heroDisplayNames, "未知英雄");
}

export function resolveMapDisplayName(mapKey: string | null | undefined, name?: string | null): string {
  return resolveDisplayName(mapKey, name, mapDisplayNames, "未知地图");
}

export function normalizeStadiumAssetPath(value: string | null | undefined): string | null {
  const raw = value?.trim();
  if (!raw) return null;
  if (raw.includes("\\") || raw.includes("%") || raw.includes(":") || /[\u0000-\u001f\u007f]/.test(raw)) {
    return null;
  }
  const prefix = "assets/stadium/";
  if (!raw.startsWith(prefix)) return null;
  const relative = raw.slice(prefix.length);
  const segments = relative.split("/");
  return segments.length > 0 && segments.every((segment) => segment !== "" && segment !== "." && segment !== "..")
    ? `${stadiumAssetRoot}/${relative}`
    : null;
}

function assetKey(value: string | null | undefined, lookup: Map<string, string>): string | null {
  const raw = value?.trim();
  if (!raw) return null;
  const normalized = normalizeAssetKey(raw);
  return lookup.get(raw.toLowerCase()) ?? (normalized ? lookup.get(normalized) ?? normalized : null);
}

function uniqueUrls(values: Array<string | null | undefined>, lookup: Map<string, string>, folder: "heroes" | "maps", extension: "png" | "jpg") {
  const seen = new Set<string>();
  const urls: string[] = [];
  for (const value of values) {
    const stadiumFile = folder === "maps" ? stadiumMapPreviewFiles.get(value?.trim() ?? "") : null;
    if (stadiumFile && !seen.has(stadiumFile)) {
      seen.add(stadiumFile);
      urls.push(`${repoAssetRoot}/maps/${stadiumFile}`);
      continue;
    }
    const key = assetKey(value, lookup);
    if (!key || seen.has(key)) continue;
    seen.add(key);
    urls.push(`${repoAssetRoot}/${folder}/${key}.${extension}`);
  }
  return urls;
}

export function heroAssetUrl(heroKey: string | null | undefined): string | null {
  return heroAssetUrls([heroKey])[0] ?? null;
}

export function heroAssetUrls(values: Array<string | null | undefined>): string[] {
  return uniqueUrls(values, heroLookup, "heroes", "png");
}

export function mapAssetUrl(mapKey: string | null | undefined): string | null {
  return mapAssetUrls([mapKey])[0] ?? null;
}

export function mapAssetUrls(values: Array<string | null | undefined>): string[] {
  return uniqueUrls(values, mapLookup, "maps", "jpg");
}

export function rankAssetUrl(rankKey: string | null | undefined): string {
  const key = normalizeAssetKey(rankKey) ?? "unranked";
  const file = rankFiles.get(key) ?? rankFiles.get("unranked") ?? "unranked.png";
  return `${rankAssetRoot}/${file}`;
}

export function arenaRankAssetUrl(rankKey: string | null | undefined): string | null {
  const key = normalizeAssetKey(rankKey);
  if (!key) return null;
  const file = arenaRankFiles.get(key);
  return file ?? null;
}

export function achievementBadgeUrl(achievementKey: string, level: 1 | 2 | 3): string {
  return `${achievementAssetRoot}/${achievementKey}-token-tier-${level}.png`;
}

export function resolvePerkInfo(rawGuid: string | number | null | undefined): PerkInfo | null {
  const keys = perkLookupKeys(rawGuid);
  for (const key of keys) {
    const perk = perkLookup.get(key);
    if (perk) return perk;
  }
  return null;
}

export function perkAssetUrls(values: Array<string | number | null | undefined>): string[] {
  const seen = new Set<string>();
  const urls: string[] = [];
  for (const value of values) {
    const iconUrl = resolvePerkInfo(value)?.iconUrl;
    if (!iconUrl || seen.has(iconUrl)) continue;
    seen.add(iconUrl);
    urls.push(iconUrl);
  }
  return urls;
}

export function resolveHeroStatInfo(guid: string | number | null | undefined): HeroStatInfo | null {
  const key = guid == null ? "" : String(guid).trim();
  if (!key) return null;
  return heroStatLookup.get(key) ?? null;
}

export type ArenaModTraitInfo = {
  id: string;
  name: string;
  description: string | null;
  iconUrl: string | null;
  category: string | null;
};

export function resolveArenaModTraitInfo(rawGuid: string | number | null | undefined): ArenaModTraitInfo | null {
  const key = rawGuid == null ? "" : String(rawGuid).trim();
  if (!key) return null;
  return arenaModTraitLookup.get(key) ?? arenaModTraitLookup.get(decimalToHexKey(key)) ?? null;
}

function buildArenaModTraitLookup(entries: StadiumModTraitEntry[]) {
  const lookup = new Map<string, ArenaModTraitInfo>();
  for (const entry of entries) {
    const keys = [entry.id, entry.decimalId, entry.hexId].filter((value): value is string => Boolean(value?.trim()));
    if (keys.length === 0) continue;
    const iconUrl = normalizeStadiumAssetPath(entry.localPath);
    const firstKey = keys[0];
    const info: ArenaModTraitInfo = {
      id: firstKey,
      name: entry.name?.trim() || "待补充映射",
      description: entry.description?.trim() || entry.desc?.trim() || null,
      iconUrl,
      category: entry.category ?? null,
    };
    for (const key of keys) lookup.set(key, info);
  }
  return lookup;
}

function decimalToHexKey(value: string): string {
  if (!/^\d+$/.test(value)) return value;
  try {
    return `0x${BigInt(value).toString(16).toUpperCase().padStart(16, "0")}`;
  } catch {
    return value;
  }
}

function buildPerkLookup(config: QueryToolConfig, iconLookup: Map<string, string>) {
  const lookup = new Map<string, PerkInfo>();
  const addPerk = (item: QueryToolPerk) => {
    const id = normalizeHexId(item.id);
    const guid = item.guid?.trim() || (id ? decimalStringFromHex(id) : "");
    if (!id && !guid) return;
    const parsedName = splitPerkName(item.name, item.desc);
    const perk: PerkInfo = {
      id: id ?? guid,
      guid,
      name: parsedName.name,
      description: parsedName.description,
      iconUrl: localPerkIconUrl(id, guid, iconLookup),
    };
    for (const key of [id, guid, guid ? hexStringFromDecimal(guid) : null]) {
      if (key) lookup.set(key.toLowerCase(), perk);
    }
  };

  for (const perks of Object.values(config.heroPerkList ?? {})) {
    for (const perk of perks) addPerk(perk);
  }
  for (const perk of config.perkList ?? []) addPerk(perk);
  return lookup;
}

function buildHeroStatLookup(config: QueryToolConfig, cache: OwConfigCache) {
  const lookup = new Map<string, HeroStatInfo>();
  for (const [guid, item] of Object.entries(cache.attrs ?? {})) {
    const label = item.text?.trim();
    if (!guid || !label) continue;
    lookup.set(guid, {
      guid,
      label,
      type: item.type?.trim() || null,
    });
  }
  for (const item of config.heroAttrList ?? []) {
    const guid = item.valueGuid?.trim();
    const label = item.valueText?.trim();
    if (!guid || !label) continue;
    if (!lookup.has(guid)) {
      lookup.set(guid, {
        guid,
        label,
        type: item.valueType?.trim() || null,
      });
    }
  }
  return lookup;
}

function buildPerkIconLookup(entries: ManifestEntry[]) {
  const lookup = new Map<string, string>();
  for (const entry of entries) {
    const url = `${repoAssetRoot}/perks/${entry.key}.png`;
    for (const key of [entry.key, entry.name, entry.cn_name, entry.source_url, ...(entry.aliases ?? [])]) {
      if (!key) continue;
      lookup.set(key.trim().toLowerCase(), url);
    }
    const guid = entry.local_path ? entry.local_path.match(/(0x[0-9a-f]+)\.png/i)?.[1] : null;
    if (guid) lookup.set(guid.toLowerCase(), url);
  }
  return lookup;
}

function localPerkIconUrl(id: string | null, guid: string, iconLookup: Map<string, string>) {
  for (const key of [id, guid, guid ? hexStringFromDecimal(guid) : null]) {
    if (!key) continue;
    const url = iconLookup.get(key.toLowerCase());
    if (url) return url;
  }
  return null;
}

function perkLookupKeys(rawGuid: string | number | null | undefined) {
  const raw = rawGuid == null ? "" : String(rawGuid).trim();
  if (!raw) return [];
  const keys = [raw.toLowerCase()];
  const hex = raw.startsWith("0x") || raw.startsWith("0X") ? normalizeHexId(raw) : hexStringFromDecimal(raw);
  if (hex) keys.push(hex.toLowerCase());
  return keys;
}

function splitPerkName(rawName: string | undefined, rawDescription: string | undefined) {
  const [namePart, descriptionPart] = (rawName ?? "").split(/<br\s*\/?>/i);
  const name = stripHtml(namePart).trim() || "未知威能";
  const description = stripHtml(rawDescription ?? descriptionPart ?? "").trim();
  return { name, description: description || null };
}

function stripHtml(value: string) {
  return value.replace(/<[^>]*>/g, "");
}

function normalizeHexId(value: string | undefined) {
  const raw = value?.trim();
  if (!raw) return null;
  if (!raw.startsWith("0x") && !raw.startsWith("0X")) return null;
  try {
    return `0x${BigInt(raw).toString(16).toUpperCase().padStart(16, "0")}`;
  } catch {
    return raw;
  }
}

function hexStringFromDecimal(value: string) {
  if (!/^\d+$/.test(value)) return null;
  try {
    return `0x${BigInt(value).toString(16).toUpperCase().padStart(16, "0")}`;
  } catch {
    return null;
  }
}

function decimalStringFromHex(value: string) {
  try {
    return BigInt(value).toString(10);
  } catch {
    return "";
  }
}
