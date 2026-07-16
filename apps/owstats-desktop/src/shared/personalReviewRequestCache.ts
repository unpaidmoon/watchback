import { getPersonalSeasonReview } from "./api";
import type { PersonalSeasonReview } from "./types";

const inFlightReviewRequests = new Map<string, Promise<PersonalSeasonReview>>();

export function requestPersonalSeasonReviewOnce(
  season: number | undefined,
  load: (requestedSeason?: number) => Promise<PersonalSeasonReview> = getPersonalSeasonReview,
) {
  const requestKey = season === undefined ? "latest" : `season:${season}`;
  const existingRequest = inFlightReviewRequests.get(requestKey);
  if (existingRequest) return existingRequest;

  const pendingRequest = load(season);
  const request = pendingRequest.finally(() => {
    if (inFlightReviewRequests.get(requestKey) === request) {
      inFlightReviewRequests.delete(requestKey);
    }
  });
  inFlightReviewRequests.set(requestKey, request);
  return request;
}

export function clearPersonalReviewRequestCache() {
  inFlightReviewRequests.clear();
}
