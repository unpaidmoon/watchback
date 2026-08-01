import stadiumRuntime from "../../runtime-config/stadium-assets.json";

import { normalizeAssetKey } from "./assets";

const stadiumAssetRoot = "/assets/stadium";

type StadiumRuntimeManifest = {
  fightRanks: StadiumRankEntry[];
  modTrait: StadiumModTraitEntry[];
};

type StadiumRankEntry = {
  key?: string;
  localPath?: string;
};

type StadiumModTraitEntry = {
  id?: string;
  decimalId?: string;
  hexId?: string;
  name?: string;
  description?: string;
  localPath?: string;
  category?: string;
};

export type ArenaModTraitInfo = {
  id: string;
  name: string;
  description: string | null;
  iconUrl: string | null;
  category: string | null;
};

const stadium = stadiumRuntime as StadiumRuntimeManifest;
const arenaRankFiles = new Map(
  stadium.fightRanks.flatMap((rank) => {
    const key = normalizeAssetKey(rank.key);
    const url = normalizeStadiumAssetPath(rank.localPath);
    return key && url ? [[key, url] as const] : [];
  }),
);
const arenaModTraitLookup = buildArenaModTraitLookup(stadium.modTrait);

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

export function arenaRankAssetUrl(rankKey: string | null | undefined): string | null {
  const key = normalizeAssetKey(rankKey);
  if (!key) return null;
  return arenaRankFiles.get(key) ?? null;
}

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
    const info: ArenaModTraitInfo = {
      id: keys[0],
      name: entry.name?.trim() || "待补充映射",
      description: entry.description?.trim() || null,
      iconUrl: normalizeStadiumAssetPath(entry.localPath),
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
