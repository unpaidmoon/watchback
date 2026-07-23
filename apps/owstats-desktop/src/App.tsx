import { Sparkles } from "lucide-react";
import { Navigate, NavLink, Route, Routes, useSearchParams } from "react-router-dom";
import { CareerMmrPage } from "./pages/CareerMmrPage";
import { FriendDetailPage } from "./pages/FriendDetailPage";
import { MatchesPage } from "./pages/MatchesPage";
import { OnboardingPage } from "./pages/OnboardingPage";
import { Phase6StyleBoardPage } from "./pages/Phase6StyleBoardPage";
import { PersonalReviewPage } from "./pages/PersonalReviewPage";
import { AchievementsPage } from "./pages/AchievementsPage";
import { RankingPage } from "./pages/RankingPage";
import { QaFeedbackProvider, QaRegion } from "./shared/QaFeedbackLayer";
import { useUnsavedChangesGuard } from "./shared/UnsavedChangesGuard";
import { RouteFocusManager } from "./shared/page-shell/RouteFocusManager";
import { phase6NavItems } from "./shared/phase6Shell";
import { buildReviewHref, legacyReviewRedirectHref, parsePositiveSeasonParam } from "./features/review/reviewNavigation";

function PersonalReviewRoute() {
  const [params] = useSearchParams();
  const redirect = legacyReviewRedirectHref(params);
  return redirect ? <Navigate to={redirect} replace /> : <PersonalReviewPage />;
}

function FriendAnalyticsRoute() {
  const [params] = useSearchParams();
  const season = parsePositiveSeasonParam(params.get("season"));
  return <Navigate to={season === undefined ? "/review" : buildReviewHref(season)} replace />;
}

export function App() {
  const { guardNavigation } = useUnsavedChangesGuard();
  return (
    <QaFeedbackProvider>
      <RouteFocusManager />
      <main className="appFrame appShell phase6ProductShell">
        <header className="appTopBar phase6ProductTopBar">
          <div className="appBrand phase6ProductBrand">
            <span className="phase6ProductLogo">
              <Sparkles size={18} />
            </span>
            <div>
              <strong className="appBrandName">OWStatsLocal</strong>
              <p>本地优先的守望先锋战绩工作台</p>
            </div>
          </div>

          <QaRegion id="NAV-CAPSULE" className="phase6QaInlineRegion">
            <nav className="appNav phase6ProductNav" aria-label="主导航">
              {phase6NavItems.map((item) => (
                <NavLink key={item.path} to={item.path} className={({ isActive }) => (isActive ? "appNavLink active" : "appNavLink")} title={item.title} onClick={guardNavigation}>
                  {item.label}
                </NavLink>
              ))}
            </nav>
          </QaRegion>
        </header>

        <div className="appCanvas phase6ProductSurface">
          <Routes>
            <Route path="/" element={<Navigate to="/matches" replace />} />
            <Route path="/onboarding" element={<OnboardingPage />} />
            <Route path="/phase6-style-board" element={<Phase6StyleBoardPage />} />
            <Route path="/matches" element={<MatchesPage />} />
            <Route path="/review" element={<PersonalReviewRoute />} />
            <Route path="/achievements" element={<AchievementsPage />} />
            <Route path="/career-mmr" element={<CareerMmrPage />} />
            <Route path="/ranking" element={<RankingPage />} />
            <Route path="/analytics/friends" element={<FriendAnalyticsRoute />} />
            <Route path="/analytics/friends/detail" element={<FriendDetailPage />} />
          </Routes>
        </div>
      </main>
    </QaFeedbackProvider>
  );
}
