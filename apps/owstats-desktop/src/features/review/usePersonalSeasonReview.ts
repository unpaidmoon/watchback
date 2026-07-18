import { useEffect, useRef, useState } from "react";
import { requestPersonalSeasonReviewOnce } from "../../shared/personalReviewRequestCache";
import { productErrorMessage } from "../../shared/phase6Shell";
import type { PersonalSeasonReview } from "../../shared/types";

export function createLatestRequestGuard() {
  let latestRequest = 0;
  return {
    begin: () => ++latestRequest,
    isCurrent: (request: number) => latestRequest === request,
    invalidate: (request?: number) => { if (request === undefined || latestRequest === request) ++latestRequest; },
  };
}

export function hasReviewSeasonData(review: Pick<PersonalSeasonReview, "summary">): review is Pick<PersonalSeasonReview, "summary"> & { summary: PersonalSeasonReview["summary"] & { season: number } } {
  return typeof review.summary.season === "number" && review.summary.season > 0;
}

export function shouldRequestReviewSeason(selectedSeason: number | undefined, loadedSeason: number | null) {
  return selectedSeason === undefined || selectedSeason !== loadedSeason;
}

export function buildReviewSeasonFallbackParams(current: URLSearchParams, backendSeason: number) {
  const next = new URLSearchParams(current);
  next.set("season", String(backendSeason));
  return next;
}

export function buildReviewSeasonFallbackFromLatest(latest: { current: URLSearchParams }, backendSeason: number) {
  return buildReviewSeasonFallbackParams(latest.current, backendSeason);
}

export function usePersonalSeasonReview(params: URLSearchParams, setParams: (next: URLSearchParams, options?: { replace?: boolean }) => void, requestedSeason: number | undefined) {
  const latestParamsRef = useRef(params);
  latestParamsRef.current = params;
  const [selectedSeason, setSelectedSeason] = useState<number | undefined>(requestedSeason);
  const [review, setReview] = useState<PersonalSeasonReview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const guard = useRef(createLatestRequestGuard());
  const loadedReviewSeasonRef = useRef<number | null>(null);

  useEffect(() => {
    if (requestedSeason === selectedSeason) return;
    guard.current.invalidate();
    loadedReviewSeasonRef.current = null;
    setSelectedSeason(requestedSeason);
    setReview(null);
    setError(null);
  }, [requestedSeason, selectedSeason]);

  function requestReview() {
    if (!shouldRequestReviewSeason(selectedSeason, loadedReviewSeasonRef.current)) return;
    const request = guard.current.begin();
    setReview(null);
    setError(null);
    void requestPersonalSeasonReviewOnce(selectedSeason).then((nextReview) => {
      if (!guard.current.isCurrent(request)) return;
      setReview(nextReview);
      const backendSeason = nextReview.summary.season;
      loadedReviewSeasonRef.current = typeof backendSeason === "number" && backendSeason > 0 ? backendSeason : null;
      if (typeof backendSeason === "number" && backendSeason > 0 && backendSeason !== selectedSeason) {
        setSelectedSeason(backendSeason);
        const nextParams = buildReviewSeasonFallbackFromLatest(latestParamsRef, backendSeason);
        setParams(nextParams, { replace: true });
      }
    }).catch((reason) => { if (guard.current.isCurrent(request)) setError(productErrorMessage(reason)); });
    return () => guard.current.invalidate(request);
  }

  useEffect(() => requestReview(), [selectedSeason]);

  function changeSeason(nextSeason: number) {
    setSelectedSeason(nextSeason);
    setReview(null);
    const next = new URLSearchParams(params);
    next.set("season", String(nextSeason));
    setParams(next);
  }

  return { selectedSeason, review, error, availableSeasons: review?.availableSeasons ?? [], requestReview, changeSeason };
}
