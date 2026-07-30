export type SeasonDataRevisionEvent = {
  accountScope: string;
  season: number;
  revision: number;
  phase: "list" | "detail" | "completed";
  newMatches: number;
  pendingDetails: number;
  status: "running" | "completed" | "partialFailure";
};

export type LiveMatchTimelineDependencies = {
  listen: (listener: (event: SeasonDataRevisionEvent) => void) => Promise<() => void>;
  getActiveAccountScope: () => Promise<string | null>;
  schedule?: (callback: () => void, delayMs: number) => unknown;
  cancel?: (timer: unknown) => void;
};

export function chooseLiveTimelineRefresh(event: SeasonDataRevisionEvent, selectedSeason: number | undefined, atTop: boolean) {
  if (selectedSeason !== undefined && selectedSeason !== event.season) return "ignore" as const;
  if (event.phase === "completed" && event.newMatches === 0) return "statusOnly" as const;
  if (event.phase === "list" && event.newMatches > 0 && !atTop) return "queue" as const;
  return "refresh" as const;
}

export function mergeLiveTimelinePage<T extends { matchId: string }>(
  current: T[],
  incoming: T[],
  phase: SeasonDataRevisionEvent["phase"],
) {
  const incomingById = new Map(incoming.map((match) => [match.matchId, match]));
  if (phase === "detail") {
    return current.map((match) => incomingById.get(match.matchId) ?? match);
  }
  const incomingIds = new Set(incoming.map((match) => match.matchId));
  return [...incoming, ...current.filter((match) => !incomingIds.has(match.matchId))];
}

export function createLiveMatchTimelineController(dependencies: LiveMatchTimelineDependencies) {
  const schedule = dependencies.schedule ?? ((callback, delayMs) => setTimeout(callback, delayMs));
  const cancel = dependencies.cancel ?? ((timer) => clearTimeout(timer as ReturnType<typeof setTimeout>));
  const revisions = new Map<string, number>();
  const latestEvents = new Map<string, SeasonDataRevisionEvent>();
  const pending = new Map<string, SeasonDataRevisionEvent>();
  const timers = new Map<string, unknown>();
  const subscribers = new Set<(event: SeasonDataRevisionEvent) => void>();
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
    pending.set(key, event);
    const existingTimer = timers.get(key);
    if (existingTimer !== undefined) cancel(existingTimer);
    const delayMs = event.phase === "detail" ? 400 : 150;
    timers.set(key, schedule(() => {
      timers.delete(key);
      const latest = pending.get(key);
      pending.delete(key);
      if (!latest || latest.revision <= (revisions.get(key) ?? 0)) return;
      revisions.set(key, latest.revision);
      latestEvents.set(key, latest);
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
    subscribe(subscriber: (event: SeasonDataRevisionEvent) => void) {
      subscribers.add(subscriber);
      return () => subscribers.delete(subscriber);
    },
    getRevision(accountScope: string, season: number) {
      return revisions.get(keyOf(accountScope, season)) ?? 0;
    },
    getLatestEvents(accountScope: string) {
      return [...latestEvents.values()].filter((event) => event.accountScope === accountScope);
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
