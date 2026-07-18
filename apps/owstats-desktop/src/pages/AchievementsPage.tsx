import { useNavigate, useSearchParams } from "react-router-dom";
import { AchievementWall } from "../features/review/AchievementWall";
import { hasReviewSeasonData, usePersonalSeasonReview } from "../features/review/usePersonalSeasonReview";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState, PageSeasonSelect } from "../shared/page-shell";
import { parseReviewSeasonParam } from "./PersonalReviewPage";

export function AchievementsPage() {
  const [params, setParams] = useSearchParams(); const navigate = useNavigate();
  const state = usePersonalSeasonReview(params, setParams, parseReviewSeasonParam(params.get("season")));
  const header = <PageHeader title="成就" description={state.selectedSeason ? `S${state.selectedSeason} 赛季成就与进度` : "查看赛季成就与进度"} actions={<PageSeasonSelect value={state.selectedSeason} seasons={state.availableSeasons} disabled={state.availableSeasons.length === 0} onChange={(season) => { if (season !== undefined) state.changeSeason(season); }} />} />;
  if (state.error) return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header}<PageErrorState message={state.error} onRetry={() => void state.requestReview()} /></section>;
  if (!state.review) return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header}<PageLoadingState message="正在加载个人赛季战报..." /></section>;
  if (!hasReviewSeasonData(state.review)) return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header}<PageEmptyState title="暂无可用赛季成就" description="当前本地数据库没有可用于复盘的赛季数据。" actionLabel="前往指挥台" onAction={() => navigate("/onboarding")} /></section>;
  return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header}<AchievementWall review={state.review} /></section>;
}
