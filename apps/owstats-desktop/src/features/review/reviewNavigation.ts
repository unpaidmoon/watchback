export type ReviewReturnContext = {
  href: string;
  scrollKey: string;
  scrollY: number;
};

export type FriendDetailLocationState = {
  bnetId: string;
  season: number;
  returnTo: ReviewReturnContext;
};

export function buildReviewHref(season: number) {
  const params = new URLSearchParams({ season: String(season) });
  return `/review?${params.toString()}`;
}

export function buildAchievementsHref(_season?: number) {
  return "/achievements";
}

export function parsePositiveSeasonParam(value: string | null) {
  if (!value) return undefined;
  const season = Number(value);
  return Number.isInteger(season) && season > 0 ? season : undefined;
}

export function legacyReviewRedirectHref(params: URLSearchParams) {
  const view = params.get("view");
  if (view === null) return null;
  const season = parsePositiveSeasonParam(params.get("season"));
  const suffix = season === undefined ? "" : `?season=${season}`;
  return view === "achievements" ? "/achievements" : `/review${suffix}`;
}

export function buildReviewRankingHref(season: number) {
  return `/ranking?tab=maps&season=${season}`;
}

export function reviewScrollKey(href: string) {
  return `review-scroll:${href}`;
}

export type ReviewScrollRuntime = {
  requestFrame: (callback: () => void) => number;
  cancelFrame: (frame: number) => void;
  scrollTo: (top: number) => void;
  readScrollY: () => number;
  readMaxScrollY: () => number;
  removeStored: () => void;
};

export function scheduleReviewScrollRestore(scrollY: number, runtime: ReviewScrollRuntime, maxAttempts = 8) {
  let frame = 0;
  let attempts = 0;
  let cancelled = false;

  const restore = () => {
    if (cancelled) return;
    attempts += 1;
    runtime.scrollTo(scrollY);
    const maxScrollY = Math.max(0, runtime.readMaxScrollY());
    const reachedTarget = scrollY === 0 || (maxScrollY >= scrollY - 1 && Math.abs(runtime.readScrollY() - scrollY) <= 1);
    if (reachedTarget || attempts >= maxAttempts) {
      runtime.removeStored();
      return;
    }
    frame = runtime.requestFrame(restore);
  };

  frame = runtime.requestFrame(restore);
  return () => {
    cancelled = true;
    runtime.cancelFrame(frame);
  };
}

export function buildFriendDetailState(
  bnetId: string,
  season: number,
  scrollY: number,
): FriendDetailLocationState {
  const href = buildReviewHref(season);
  return {
    bnetId,
    season,
    returnTo: { href, scrollKey: reviewScrollKey(href), scrollY },
  };
}

export function parseFriendDetailState(value: unknown): FriendDetailLocationState | null {
  if (!value || typeof value !== "object") return null;
  const candidate = value as Partial<FriendDetailLocationState>;
  if (typeof candidate.bnetId !== "string" || candidate.bnetId.length === 0) return null;
  if (typeof candidate.season !== "number" || !Number.isInteger(candidate.season) || candidate.season <= 0) return null;
  if (!candidate.returnTo || typeof candidate.returnTo !== "object") return null;

  const { href, scrollKey, scrollY } = candidate.returnTo;
  if (typeof href !== "string" || typeof scrollKey !== "string" || typeof scrollY !== "number" || !Number.isFinite(scrollY) || scrollY < 0) {
    return null;
  }

  let parsed: URL;
  try {
    parsed = new URL(href, "http://owstats.local");
  } catch {
    return null;
  }
  if (`${parsed.pathname}${parsed.search}` !== buildReviewHref(candidate.season)) return null;
  if (scrollKey !== reviewScrollKey(href)) return null;

  return candidate as FriendDetailLocationState;
}
