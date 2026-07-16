import { clearMatchesPageCache } from "./matchesPageCache";
import { clearPersonalReviewRequestCache } from "./personalReviewRequestCache";

export function clearAccountScopedFrontendCaches() {
  clearMatchesPageCache();
  clearPersonalReviewRequestCache();
}
