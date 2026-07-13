export const reviewViews = ["overview", "roles", "friends", "maps", "achievements"] as const;

export type ReviewView = (typeof reviewViews)[number];

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

export function parseReviewView(value: string | null): ReviewView {
  return reviewViews.includes(value as ReviewView) ? (value as ReviewView) : "overview";
}

export function buildReviewHref(season: number, view: ReviewView) {
  const params = new URLSearchParams({ season: String(season), view });
  return `/review?${params.toString()}`;
}

export function buildReviewRankingHref(season: number) {
  return `/ranking?tab=maps&season=${season}`;
}

export function reviewScrollKey(href: string) {
  return `review-scroll:${href}`;
}

export function buildFriendDetailState(
  bnetId: string,
  season: number,
  view: ReviewView,
  scrollY: number,
): FriendDetailLocationState {
  const href = buildReviewHref(season, view);
  return {
    bnetId,
    season,
    returnTo: { href, scrollKey: reviewScrollKey(href), scrollY },
  };
}
