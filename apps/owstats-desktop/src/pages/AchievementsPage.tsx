import { useNavigate } from "react-router-dom";
import { AchievementWall } from "../features/review/AchievementWall";
import { useCareerAchievements } from "../features/review/useCareerAchievements";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState } from "../shared/page-shell";

export function AchievementsPage() {
  const navigate = useNavigate(); const state = useCareerAchievements();
  const header = <PageHeader title="成就" description="生涯成就与进度" />;
  if (state.error) return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header}<PageErrorState message={state.error} onRetry={state.requestAchievements} /></section>;
  if (state.loading || !state.achievements) return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header}<PageLoadingState message="正在加载生涯成就..." /></section>;
  if (state.achievements.length === 0) return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header}<PageEmptyState title="暂无生涯成就" description="当前本地数据库没有可用于计算成就的对局数据。" actionLabel="前往指挥台" onAction={() => navigate("/onboarding")} /></section>;
  return <section className="workspace reviewWorkspace phase6ReviewWorkspace">{header}<AchievementWall achievements={state.achievements} /></section>;
}
