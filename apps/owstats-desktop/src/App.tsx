import { BarChart3, RefreshCw, ShieldCheck, Sparkles, Trophy } from "lucide-react";
import { Navigate, NavLink, Route, Routes } from "react-router-dom";
import { FriendAnalyticsPage } from "./pages/FriendAnalyticsPage";
import { FriendDetailPage } from "./pages/FriendDetailPage";
import { MatchDetailPage } from "./pages/MatchDetailPage";
import { MatchesPage } from "./pages/MatchesPage";
import { OnboardingPage } from "./pages/OnboardingPage";
import { PersonalReviewPage } from "./pages/PersonalReviewPage";
import { RankingPage } from "./pages/RankingPage";
import { SyncPage } from "./pages/SyncPage";

export function App() {
  return (
    <main className="appShell">
      <header className="topBar">
        <div>
          <h1>OWStatsLocal</h1>
          <p>本地优先的 Overwatch 战绩复盘</p>
        </div>
        <nav>
          <NavLink to="/onboarding" className={({ isActive }) => `iconButton ${isActive ? "active" : ""}`} title="首次设置">
            <Sparkles size={18} />
            设置
          </NavLink>
          <NavLink to="/matches" className={({ isActive }) => `iconButton ${isActive ? "active" : ""}`} title="对局">
            <ShieldCheck size={18} />
            对局
          </NavLink>
          <NavLink to="/review" className={({ isActive }) => `iconButton ${isActive ? "active" : ""}`} title="个人复盘">
            <BarChart3 size={18} />
            复盘
          </NavLink>
          <NavLink to="/ranking" className={({ isActive }) => `iconButton ${isActive ? "active" : ""}`} title="排行">
            <Trophy size={18} />
            排行
          </NavLink>
          <NavLink to="/analytics/friends" className={({ isActive }) => `iconButton ${isActive ? "active" : ""}`} title="分析">
            <BarChart3 size={18} />
            分析
          </NavLink>
          <NavLink to="/sync" className={({ isActive }) => `iconButton ${isActive ? "active" : ""}`} title="同步">
            <RefreshCw size={18} />
            同步
          </NavLink>
        </nav>
      </header>

      <Routes>
        <Route path="/" element={<Navigate to="/onboarding" replace />} />
        <Route path="/onboarding" element={<OnboardingPage />} />
        <Route path="/matches" element={<MatchesPage />} />
        <Route path="/matches/:matchId" element={<MatchDetailPage />} />
        <Route path="/review" element={<PersonalReviewPage />} />
        <Route path="/ranking" element={<RankingPage />} />
        <Route path="/analytics/friends" element={<FriendAnalyticsPage />} />
        <Route path="/analytics/friends/:bnetId" element={<FriendDetailPage />} />
        <Route path="/sync" element={<SyncPage />} />
      </Routes>
    </main>
  );
}
