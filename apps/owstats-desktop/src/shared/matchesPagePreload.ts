import { getOverview, listMatchFilters, listMatchTimeline } from "./api";
import { preloadMatchesPageCache } from "./matchesPageCache";

export function preloadDefaultMatchesPage() {
  return preloadMatchesPageCache({
    getOverview,
    listMatchFilters,
    listMatchTimeline,
  });
}
