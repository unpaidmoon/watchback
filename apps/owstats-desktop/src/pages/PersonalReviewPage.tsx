import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { createLatestRequestGuard, hasReviewSeasonData, usePersonalSeasonReview } from "../features/review/usePersonalSeasonReview";
import { FriendAnalysisModules, MapAnalysisModules } from "./FriendAnalyticsPage";
import { buildReviewHref, parsePositiveSeasonParam, reviewScrollKey, scheduleReviewScrollRestore } from "../features/review/reviewNavigation";
import { getSeasonAnalysis } from "../shared/api";
import { arenaRankAssetUrl, rankAssetUrl } from "../shared/assets";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState, PageSeasonSelect } from "../shared/page-shell";
import { GlassPanel, HeroAvatar } from "../shared/phase6Components";
import { productErrorMessage } from "../shared/phase6Shell";
import { formatWinRate } from "../shared/rankingUtils";
import type { PersonalSeasonReview, RoleRankSummary, SeasonAnalysis } from "../shared/types";

export { requestPersonalSeasonReviewOnce } from "../shared/personalReviewRequestCache";
export { buildReviewSeasonFallbackFromLatest, buildReviewSeasonFallbackParams, createLatestRequestGuard, hasReviewSeasonData, shouldRequestReviewSeason } from "../features/review/usePersonalSeasonReview";

export function ReviewEmptyState({ onAction = () => undefined }: { onAction?: () => void }) { return <PageEmptyState title="暂无可用赛季战报" description="当前本地数据库没有可用于复盘的赛季数据。" actionLabel="前往指挥台" onAction={onAction} />; }

export function PersonalReviewPage() {
  const [params, setParams] = useSearchParams(); const navigate = useNavigate(); const requestedSeason = parseReviewSeasonParam(params.get("season"));
  const personal = usePersonalSeasonReview(params, setParams, requestedSeason);
  const { selectedSeason, review, error, availableSeasons, requestReview, changeSeason } = personal;
  const [analysis, setAnalysis] = useState<SeasonAnalysis | null>(null); const [analysisLoading, setAnalysisLoading] = useState(false); const [analysisError, setAnalysisError] = useState<string | null>(null); const analysisGuard = useRef(createLatestRequestGuard());
  useEffect(() => { if (selectedSeason === undefined) { setAnalysis(null); setAnalysisError(null); setAnalysisLoading(false); return; } const request = analysisGuard.current.begin(); setAnalysis(null); setAnalysisLoading(true); setAnalysisError(null); void getSeasonAnalysis(selectedSeason).then((next) => { if (analysisGuard.current.isCurrent(request)) setAnalysis(next); }).catch((reason) => { if (analysisGuard.current.isCurrent(request)) setAnalysisError(productErrorMessage(reason)); }).finally(() => { if (analysisGuard.current.isCurrent(request)) setAnalysisLoading(false); }); return () => analysisGuard.current.invalidate(request); }, [selectedSeason]);
  const header = (season: number | null | undefined) => <PageHeader title="战报" description={season ? `S${season} 赛季表现与协作复盘` : "查看赛季表现与协作复盘"} actions={<PageSeasonSelect value={selectedSeason} seasons={availableSeasons} disabled={availableSeasons.length === 0} onChange={(next) => { if (next !== undefined) changeSeason(next); }} />} />;
  if (error) return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header(selectedSeason)}<PageErrorState message={error} onRetry={() => void requestReview()} /></section>;
  if (!review) return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header(selectedSeason)}<PageLoadingState message="正在加载个人赛季战报..." /></section>;
  if (!hasReviewSeasonData(review)) return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header(selectedSeason)}<ReviewEmptyState onAction={() => navigate("/onboarding")} /></section>;
  const titleSeason = selectedSeason ?? review.summary.season;
  return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header(titleSeason)}<ReviewScrollRestorer season={titleSeason} /><div className="reviewReportFlow"><RoleAndHeroView review={review} /><FriendAnalysisModules analytics={analysis?.friend ?? null} loading={analysisLoading} error={analysisError} season={titleSeason} /><MapAnalysisModules analysis={analysis} loading={analysisLoading} error={analysisError} season={titleSeason} /></div></section>;
}

function ReviewScrollRestorer({ season }: { season: number }) { useLayoutEffect(() => { const href = buildReviewHref(season); const stored = sessionStorage.getItem(reviewScrollKey(href)); if (stored === null) return; const scrollY = Number(stored); if (!Number.isFinite(scrollY)) return; return scheduleReviewScrollRestore(scrollY, { requestFrame: requestAnimationFrame, cancelFrame: cancelAnimationFrame, scrollTo: (top) => window.scrollTo({ top }), readScrollY: () => window.scrollY, readMaxScrollY: () => Math.max(document.documentElement.scrollHeight, document.body.scrollHeight) - window.innerHeight, removeStored: () => sessionStorage.removeItem(reviewScrollKey(href)) }); }, [season]); return null; }
function RoleAndHeroView({ review }: { review: PersonalSeasonReview }) { return <GlassPanel className="phase6RoleReport reviewRoleHeroPanel" tone="solid"><div className="reviewHeroSummary" aria-label="赛季事实"><div className="reviewHeroAvatars"><span>常用英雄</span><div className="reviewHeroAvatarList">{review.heroPool.mainHeroes.slice(0, 3).map((hero) => <HeroAvatar key={hero.hero} heroKey={hero.hero} name={hero.hero} size="md" />)}</div></div><Metric label="赛季对局" value={`${review.summary.totalMatches} 场`} /><Metric label="总胜率" value={formatWinRate(review.summary.winRate, 0)} /></div><div className="reviewRoleRankGroup"><span className="phase6Eyebrow">预设竞技与开放竞技</span><div className="phase6RoleRankGrid">{review.roleSummaries.map((role) => <RoleRankCard key={role.roleType} role={role} />)}</div></div><div className="reviewRoleRankGroup"><span className="phase6Eyebrow">角斗竞技</span><div className="phase6RoleRankGrid reviewStadiumRankGrid">{review.stadiumRoleSummaries.map((role) => <RoleRankCard key={`stadium-${role.roleType}`} role={role} arena />)}</div></div></GlassPanel>; }
export function parseReviewSeasonParam(value: string | null) { return parsePositiveSeasonParam(value); }
function RoleRankCard({ role, arena = false }: { role: RoleRankSummary; arena?: boolean }) { const tier = role.rankSubTier > 0 ? String(role.rankSubTier) : ""; const icon = arena ? arenaRankAssetUrl(role.rankKey) ?? rankAssetUrl(role.rankKey) : rankAssetUrl(role.rankKey); return <article className="phase6RoleRankCard"><div className="phase6RoleRankName">{role.roleLabel}</div><div className="phase6RoleRankIconWrap"><img className="phase6RoleRankIcon" src={icon} alt={`${role.roleLabel} ${role.rankKey}`} />{tier ? <span className="phase6RoleRankTier">{tier}</span> : null}</div><div className="phase6RoleRankStats"><Metric label="胜率" value={formatWinRate(role.winRate, 0)} /><Metric label="场次" value={role.matches} /><Metric label="KDA" value={role.matches > 0 ? role.kda.toFixed(2) : "-"} /></div></article>; }
function Metric({ label, value }: { label: string; value: string | number }) { return <div className="metric"><span>{label}</span><strong>{value}</strong></div>; }
