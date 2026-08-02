export type SeasonDataRevisionEvent = {
  accountScope: string;
  season: number;
  revision: number;
  phase: "list" | "detail" | "completed";
  newMatches: number;
  pendingDetails: number;
  status: "running" | "completed" | "partialFailure";
};

export type LiveTimelineInvalidation = SeasonDataRevisionEvent & {
  requiresListMerge: boolean;
};

export type LiveMatchTimelineDependencies = {
  listen: (listener: (event: SeasonDataRevisionEvent) => void) => Promise<() => void>;
  getActiveAccountScope: () => Promise<string | null>;
  schedule?: (callback: () => void, delayMs: number) => unknown;
  cancel?: (timer: unknown) => void;
};

export function coalesceLiveTimelineRevision(
  current: LiveTimelineInvalidation | null | undefined,
  incoming: SeasonDataRevisionEvent | LiveTimelineInvalidation,
): LiveTimelineInvalidation {
  const incomingRequiresListMerge =
    ("requiresListMerge" in incoming && incoming.requiresListMerge)
    || (incoming.phase === "list" && incoming.newMatches > 0);
  const normalizedIncoming: LiveTimelineInvalidation = {
    ...incoming,
    requiresListMerge: incomingRequiresListMerge,
  };

  if (
    !current
    || current.accountScope !== incoming.accountScope
    || current.season !== incoming.season
  ) {
    return normalizedIncoming;
  }
  if (incoming.revision <= current.revision) return current;

  return {
    ...normalizedIncoming,
    newMatches: current.newMatches + incoming.newMatches,
    requiresListMerge: current.requiresListMerge || incomingRequiresListMerge,
  };
}

export function chooseLiveTimelineRefresh(event: LiveTimelineInvalidation, selectedSeason: number | undefined) {
  if (selectedSeason !== undefined && selectedSeason !== event.season) return "ignore" as const;
  if (event.phase === "completed" && !event.requiresListMerge && event.newMatches === 0) return "statusOnly" as const;
  return "refresh" as const;
}

function mergeLiveTimelinePage<T extends { matchId: string }>(
  current: T[],
  incoming: T[],
  invalidation: Pick<LiveTimelineInvalidation, "requiresListMerge">,
) {
  const incomingById = new Map(incoming.map((match) => [match.matchId, match]));
  if (!invalidation.requiresListMerge) {
    return current.map((match) => incomingById.get(match.matchId) ?? match);
  }
  const incomingIds = new Set(incoming.map((match) => match.matchId));
  return [...incoming, ...current.filter((match) => !incomingIds.has(match.matchId))];
}

export function reconcileLiveTimelinePage<T extends { matchId: string }, C>(
  current: { matches: T[]; nextCursor: C | null },
  incoming: { matches: T[]; nextCursor: C | null },
  invalidation: Pick<LiveTimelineInvalidation, "requiresListMerge">,
) {
  if (invalidation.requiresListMerge) {
    return {
      matches: incoming.matches,
      nextCursor: incoming.nextCursor,
      resetViewport: true,
    };
  }
  return {
    matches: mergeLiveTimelinePage(current.matches, incoming.matches, invalidation),
    nextCursor: current.matches.length > incoming.matches.length
      ? current.nextCursor
      : incoming.nextCursor,
    resetViewport: false,
  };
}

export function createLiveMatchTimelineController(dependencies: LiveMatchTimelineDependencies) {
  const schedule = dependencies.schedule ?? ((callback, delayMs) => setTimeout(callback, delayMs));
  const cancel = dependencies.cancel ?? ((timer) => clearTimeout(timer as ReturnType<typeof setTimeout>));
  const revisions = new Map<string, number>();
  const pending = new Map<string, LiveTimelineInvalidation>();
  const timers = new Map<string, unknown>();
  const subscribers = new Set<(event: LiveTimelineInvalidation) => void>();
  let unlisten: (() => void) | null = null;
  let generation = 0;

  const keyOf = (accountScope: string, season: number) => `${accountScope}:${season}`;
  const receive = async (event: SeasonDataRevisionEvent) => {
    const receiveGeneration = generation;
    const activeAccountScope = await dependencies.getActiveAccountScope();
    if (receiveGeneration !== generation || activeAccountScope !== event.accountScope) return;
    const key = keyOf(event.accountScope, event.season);
    const known = revisions.get(key) ?? 0;
    const queued = pending.get(key)?.revision ?? 0;
    if (event.revision <= Math.max(known, queued)) return;
    const next = coalesceLiveTimelineRevision(pending.get(key), event);
    pending.set(key, next);
    const existingTimer = timers.get(key);
    if (existingTimer !== undefined) cancel(existingTimer);
    const delayMs = next.requiresListMerge || next.phase !== "detail" ? 150 : 400;
    timers.set(key, schedule(() => {
      timers.delete(key);
      const latest = pending.get(key);
      pending.delete(key);
      if (!latest || latest.revision <= (revisions.get(key) ?? 0)) return;
      revisions.set(key, latest.revision);
      subscribers.forEach((subscriber) => subscriber(latest));
    }, delayMs));
  };

  return {
    async start() {
      if (unlisten) return;
      generation += 1;
      const activeGeneration = generation;
      const release = await dependencies.listen((event) => { void receive(event); });
      if (activeGeneration !== generation) release(); else unlisten = release;
    },
    stop() {
      generation += 1;
      unlisten?.();
      unlisten = null;
      timers.forEach(cancel);
      timers.clear();
      pending.clear();
    },
    subscribe(subscriber: (event: LiveTimelineInvalidation) => void) {
      subscribers.add(subscriber);
      return () => subscribers.delete(subscriber);
    },
    getRevision(accountScope: string, season: number) {
      return revisions.get(keyOf(accountScope, season)) ?? 0;
    },
    getRevisionToken(accountScope: string, selectedSeason?: number) {
      return [...revisions.entries()]
        .filter(([key]) => key.startsWith(`${accountScope}:`))
        .map(([key, revision]) => ({ season: Number(key.slice(accountScope.length + 1)), revision }))
        .filter(({ season }) => selectedSeason === undefined || season === selectedSeason)
        .sort((left, right) => left.season - right.season)
        .map(({ season, revision }) => `${season}:${revision}`)
        .join(";");
    },
  };
}
