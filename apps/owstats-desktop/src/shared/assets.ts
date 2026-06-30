import assetManifest from "../../../../assets/manifest.json";

const repoAssetRoot = "/@fs/D:/Codex/OW_data/owstats-local/assets";

type ManifestEntry = {
  key: string;
  name?: string;
  cn_name?: string;
  aliases?: string[];
};

type AssetManifest = {
  heroes: ManifestEntry[];
  maps: ManifestEntry[];
};

const manifest = assetManifest as AssetManifest;

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

const heroLookup = manifestLookup(manifest.heroes);
const mapLookup = new Map([...manifestLookup(manifest.maps), ...Object.entries(localizedMapAliases).map(([name, key]) => [name.toLowerCase(), key] as const)]);

export function normalizeAssetKey(value: string | null | undefined): string | null {
  const normalized = value
    ?.trim()
    .toLowerCase()
    .replace(/['’]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalized ? normalized : null;
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
