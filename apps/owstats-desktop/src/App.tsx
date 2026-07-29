import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { Navigate, NavLink, Route, Routes, useLocation, useNavigate, useSearchParams } from "react-router-dom";
import type { PlayerQueryHandoff } from "./pages/PlayerQueryPage";
import { endPlayerQuery } from "./features/player-query/playerQueryApi";
import { getUserSettings, listAccounts, saveUserSettings } from "./shared/api";
import type { UserSettings } from "./shared/types";
import { useUnsavedChangesGuard } from "./shared/UnsavedChangesGuard";
import { RouteFocusManager } from "./shared/page-shell/RouteFocusManager";
import { phase6NavItems } from "./shared/phase6Shell";
import { buildReviewHref, legacyReviewRedirectHref, parsePositiveSeasonParam } from "./features/review/reviewNavigation";

const watchbackMarkUrl = new URL("../assets/brand/watchback/watchback-mark.svg", import.meta.url).href;
const FriendDetailPage = lazy(() => import("./pages/FriendDetailPage").then((module) => ({ default: module.FriendDetailPage })));
const MatchesPage = lazy(() => import("./pages/MatchesPage").then((module) => ({ default: module.MatchesPage })));
const OnboardingPage = lazy(() => import("./pages/OnboardingPage").then((module) => ({ default: module.OnboardingPage })));
const PersonalReviewPage = lazy(() => import("./pages/PersonalReviewPage").then((module) => ({ default: module.PersonalReviewPage })));
const AchievementsPage = lazy(() => import("./pages/AchievementsPage").then((module) => ({ default: module.AchievementsPage })));
const RankingPage = lazy(() => import("./pages/RankingPage").then((module) => ({ default: module.RankingPage })));
const PlayerQueryPage = lazy(() => import("./pages/PlayerQueryPage").then((module) => ({ default: module.PlayerQueryPage })));
const PlayerQueryMatchesPage = lazy(() => import("./pages/PlayerQueryMatchesPage").then((module) => ({ default: module.PlayerQueryMatchesPage })));

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

function StartupRoute() {
  const [destination, setDestination] = useState<"/matches" | "/onboarding" | null>(null);

  useEffect(() => {
    let disposed = false;
    void listAccounts()
      .then((status) => {
        if (!disposed) setDestination(status.accounts.length === 0 ? "/onboarding" : "/matches");
      })
      .catch(() => {
        if (!disposed) setDestination("/matches");
      });
    return () => {
      disposed = true;
    };
  }, []);

  return destination ? <Navigate to={destination} replace /> : null;
}

type OperationStatus = "idle" | "account" | "foregroundSync" | "backgroundQueued" | "backgroundRunning" | "query";

export type PlayerQueryIntent = { id: number; battleTag: string };

function PlayerQueryFeatureRoutes({ queryIntent, onQueryPlayer }: { queryIntent: PlayerQueryIntent | null; onQueryPlayer: (battleTag: string) => void }) {
  const location = useLocation();
  const navigate = useNavigate();
  const navigateRef = useRef(navigate);
  const [handoff, setHandoff] = useState<PlayerQueryHandoff | null>(null);
  const [accountReady, setAccountReady] = useState(false);
  const [operation, setOperation] = useState<OperationStatus>("idle");
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [authenticationInvalid, setAuthenticationInvalid] = useState(false);
  const sessionRef = useRef<string | null>(null);
  const accountRef = useRef<string | null>(null);

  useEffect(() => {
    navigateRef.current = navigate;
  }, [navigate]);

  const refreshRuntime = useCallback(async () => {
    const [accounts, nextOperation] = await Promise.all([
      listAccounts(),
      invoke<OperationStatus>("get_operation_status"),
    ]);
    const active = accounts.accounts.find((account) => account.accountId === accounts.activeAccountId);
    const previous = accountRef.current;
    accountRef.current = accounts.activeAccountId;
    setAccountReady(Boolean(active && active.credentialState === "ready"));
    setOperation(nextOperation);
    if (previous && previous !== accounts.activeAccountId && sessionRef.current) {
      const staleSession = sessionRef.current;
      sessionRef.current = null;
      setHandoff(null);
      void endPlayerQuery(staleSession).catch(() => undefined);
      navigateRef.current("/player-query", { replace: true });
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setInterval> | undefined;
    const unlisteners: Array<() => void> = [];
    void Promise.all([refreshRuntime(), getUserSettings().then((value) => { if (!disposed) setSettings(value); })]).catch(() => {
      if (!disposed) setAccountReady(false);
    });
    timer = setInterval(() => { void refreshRuntime().catch(() => undefined); }, 1_000);
    for (const eventName of ["sync-progress", "season-background-progress", "season-background-completed", "credential-login-progress"]) {
      void listen(eventName, () => { void refreshRuntime().catch(() => undefined); }).then((unlisten) => {
        if (disposed) unlisten(); else unlisteners.push(unlisten);
      });
    }
    return () => {
      disposed = true;
      if (timer) clearInterval(timer);
      unlisteners.forEach((unlisten) => unlisten());
      const sessionId = sessionRef.current;
      sessionRef.current = null;
      if (sessionId) void endPlayerQuery(sessionId).catch(() => undefined);
    };
  }, [refreshRuntime]);

  useEffect(() => {
    if (location.pathname === "/player-query/matches" && !handoff) navigate("/player-query", { replace: true });
  }, [handoff, location.pathname, navigate]);

  const invalidateAuthentication = useCallback(() => {
    const sessionId = sessionRef.current;
    sessionRef.current = null;
    setHandoff(null);
    setAuthenticationInvalid(true);
    setAccountReady(false);
    if (sessionId) void endPlayerQuery(sessionId).catch(() => undefined);
    navigate("/player-query", { replace: true });
  }, [navigate]);

  const saveModeOrder = useCallback(async (order: readonly UserSettings["playerQueryModeOrder"][number][]) => {
    const base = settings ?? await getUserSettings();
    const saved = await saveUserSettings({ ...base, playerQueryModeOrder: [...order] });
    setSettings(saved);
  }, [settings]);

  const isMore = location.pathname === "/player-query/matches";
  const syncActive = operation === "account" || operation === "foregroundSync" || operation === "backgroundQueued" || operation === "backgroundRunning";
  return <>
    <div hidden={isMore}>
      <PlayerQueryPage
        hasActiveAccount={accountReady}
        syncActive={syncActive}
        initialModeOrder={settings?.playerQueryModeOrder}
        modeOrderReady={settings !== null}
        saveModeOrder={saveModeOrder}
        onOpenOnboarding={() => navigate("/onboarding")}
        onOpenMatches={(next) => { setHandoff(next); navigate("/player-query/matches"); }}
        onSessionChange={(sessionId) => { sessionRef.current = sessionId; if (sessionId) setAuthenticationInvalid(false); void refreshRuntime().catch(() => undefined); }}
        onAuthenticationInvalid={invalidateAuthentication}
        externallyAuthenticationInvalid={authenticationInvalid}
        queryIntent={queryIntent}
      />
    </div>
    {isMore ? <PlayerQueryMatchesPage handoff={handoff} onBack={() => { setHandoff((current) => current && current.matches[0] ? { ...current, selectedMatchId: current.matches[0].matchId } : current); navigate("/player-query"); }} onQueryPlayer={onQueryPlayer} onSessionMissing={() => navigate("/player-query", { replace: true })} onAuthenticationInvalid={invalidateAuthentication} /> : null}
  </>;
}

export function App() {
  const { guardNavigation } = useUnsavedChangesGuard();
  const navigate = useNavigate();
  const queryIntentId = useRef(0);
  const [queryIntent, setQueryIntent] = useState<PlayerQueryIntent | null>(null);
  const queryPlayer = useCallback((battleTag: string) => {
    setQueryIntent({ id: ++queryIntentId.current, battleTag });
    navigate("/player-query");
  }, [navigate]);
  return (
    <>
      <RouteFocusManager />
      <main className="appFrame appShell phase6ProductShell">
        <header className="appTopBar phase6ProductTopBar">
          <div className="appBrand phase6ProductBrand">
            <span className="phase6ProductLogo" aria-hidden="true">
              <img src={watchbackMarkUrl} alt="" />
            </span>
            <div>
              <strong className="appBrandName">Watchback</strong>
              <p>守望战绩工具</p>
            </div>
          </div>

          <div className="phase6NavRegion">
            <nav className="appNav phase6ProductNav" aria-label="主导航">
              {phase6NavItems.map((item) => (
                <NavLink key={item.path} to={item.path} className={({ isActive }) => (isActive ? "appNavLink active" : "appNavLink")} title={item.title} onClick={guardNavigation}>
                  {item.label}
                </NavLink>
              ))}
            </nav>
          </div>
        </header>

        <div className="appCanvas phase6ProductSurface">
          <Suspense fallback={<div role="status" aria-live="polite">正在加载...</div>}>
            <Routes>
              <Route path="/" element={<StartupRoute />} />
              <Route path="/onboarding" element={<OnboardingPage />} />
              <Route path="/matches" element={<MatchesPage onQueryPlayer={queryPlayer} />} />
              <Route path="/player-query/*" element={<PlayerQueryFeatureRoutes queryIntent={queryIntent} onQueryPlayer={queryPlayer} />} />
              <Route path="/review" element={<PersonalReviewRoute />} />
              <Route path="/achievements" element={<AchievementsPage />} />
              <Route path="/ranking" element={<RankingPage />} />
              <Route path="/analytics/friends" element={<FriendAnalyticsRoute />} />
              <Route path="/analytics/friends/detail" element={<FriendDetailPage />} />
            </Routes>
          </Suspense>
        </div>
      </main>
    </>
  );
}
