import { Bell, CircleDot, Sparkles } from "lucide-react";
import { useEffect } from "react";
import { Navigate, NavLink, Route, Routes } from "react-router-dom";
import { FriendAnalyticsPage } from "./pages/FriendAnalyticsPage";
import { FriendDetailPage } from "./pages/FriendDetailPage";
import { MatchDetailPage } from "./pages/MatchDetailPage";
import { MatchesPage } from "./pages/MatchesPage";
import { OnboardingPage } from "./pages/OnboardingPage";
import { Phase6StyleBoardPage } from "./pages/Phase6StyleBoardPage";
import { PersonalReviewPage } from "./pages/PersonalReviewPage";
import { RankingPage } from "./pages/RankingPage";
import { SyncPage } from "./pages/SyncPage";
import { QaFeedbackProvider, QaRegion } from "./shared/QaFeedbackLayer";
import { StatusBadge } from "./shared/phase6Components";
import { phase6NavItems } from "./shared/phase6Shell";
import { preloadDefaultMatchesPage } from "./shared/matchesPagePreload";

export function App() {
  useEffect(() => {
    void preloadDefaultMatchesPage().catch(() => undefined);
  }, []);

  return (
    <QaFeedbackProvider>
      <main className="appShell phase6ProductShell">
        <header className="phase6ProductTopBar">
          <div className="phase6ProductBrand">
            <span className="phase6ProductLogo">
              <Sparkles size={18} />
            </span>
            <div>
              <h1>OWStatsLocal</h1>
              <p>本地优先的守望先锋战绩工作台</p>
            </div>
          </div>

          <QaRegion id="NAV-CAPSULE" className="phase6QaInlineRegion">
            <nav className="phase6ProductNav" aria-label="主导航">
              {phase6NavItems.map((item) => (
                <NavLink key={item.path} to={item.path} className={({ isActive }) => (isActive ? "active" : "")} title={item.title}>
                  {item.label}
                </NavLink>
              ))}
            </nav>
          </QaRegion>

          <div className="phase6ShellStatus" aria-label="全局状态">
            <StatusBadge>
              <CircleDot size={14} /> 本地数据
            </StatusBadge>
            <button className="phase6ShellIconButton" type="button" aria-label="通知">
              <Bell size={17} />
            </button>
          </div>
        </header>

        <div className="phase6ProductSurface">
          <Routes>
            <Route path="/" element={<Navigate to="/onboarding" replace />} />
            <Route path="/onboarding" element={<OnboardingPage />} />
            <Route path="/phase6-style-board" element={<Phase6StyleBoardPage />} />
            <Route path="/matches" element={<MatchesPage />} />
            <Route path="/matches/:matchId" element={<MatchDetailPage />} />
            <Route path="/review" element={<PersonalReviewPage />} />
            <Route path="/ranking" element={<RankingPage />} />
            <Route path="/analytics/friends" element={<FriendAnalyticsPage />} />
            <Route path="/analytics/friends/:bnetId" element={<FriendDetailPage />} />
            <Route path="/sync" element={<SyncPage />} />
          </Routes>
        </div>
      </main>
    </QaFeedbackProvider>
  );
}
