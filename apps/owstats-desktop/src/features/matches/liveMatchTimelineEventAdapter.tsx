import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { listAccounts } from "../../shared/api";
import { hasDesktopBackend } from "../../shared/phase6Shell";
import { createLiveMatchTimelineController, type SeasonDataRevisionEvent } from "./liveMatchTimeline";

let activeAccountScope: string | null = null;
async function resolveActiveAccountScope() {
  const status = await listAccounts();
  activeAccountScope = status.activeAccountId;
  return activeAccountScope;
}

const controller = createLiveMatchTimelineController({
  listen: async (listener) => {
    if (!hasDesktopBackend()) return () => undefined;
    return listen<SeasonDataRevisionEvent>("season-data-revision", (event) => listener(event.payload));
  },
  getActiveAccountScope: resolveActiveAccountScope,
});

export function LiveMatchTimelineRoot() {
  useEffect(() => {
    void resolveActiveAccountScope().catch(() => { activeAccountScope = null; });
    void controller.start();
    return () => controller.stop();
  }, []);
  return null;
}

export function subscribeLiveMatchTimeline(listener: (event: SeasonDataRevisionEvent) => void) {
  return controller.subscribe(listener);
}

export async function getLiveMatchTimelineHandshake() {
  const accountScope = activeAccountScope ?? await resolveActiveAccountScope();
  return { accountScope, events: accountScope ? controller.getLatestEvents(accountScope) : [] };
}

export function getLiveMatchTimelineRevisionToken(accountScope: string, selectedSeason?: number) {
  return controller.getRevisionToken(accountScope, selectedSeason);
}
