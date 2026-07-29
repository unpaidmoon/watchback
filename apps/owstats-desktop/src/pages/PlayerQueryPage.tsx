import { useEffect, useRef, useState } from "react";
import type { PlayerQueryIntent } from "../App";
import { PlayerHeroUsage } from "../features/player-query/PlayerHeroUsage";
import { PlayerProfileCard } from "../features/player-query/PlayerProfileCard";
import { PlayerQueryForm } from "../features/player-query/PlayerQueryForm";
import { PlayerQueryMatchList } from "../features/player-query/PlayerQueryMatchList";
import { PlayerQueryModeTabs } from "../features/player-query/PlayerQueryModeTabs";
import { normalizeModeOrder, reorderModesForDrop } from "../features/player-query/modeOrder";
import { PLAYER_QUERY_SEASONS, type PlayerQueryListScopeKey, type PlayerQueryMatchRow, type PlayerQueryMode, type PlayerQuerySeason } from "../features/player-query/playerQueryTypes";
import { usePlayerQuerySession } from "../features/player-query/usePlayerQuerySession";
import { RoleRankOverview, type RoleRankOverviewEntry } from "../features/review/RoleRankOverview";
import { GlassPanel } from "../shared/phase6Components";
import { PageSeasonSelect } from "../shared/page-shell";
import "../styles/features/player-query.css";

export type PlayerQueryHandoff = { sessionId: string; battleTag: string; season: number; mode: PlayerQueryMode; listScopeKey: PlayerQueryListScopeKey; matches: PlayerQueryMatchRow[]; endReached: boolean; selectedMatchId: string };
export type PlayerQueryPageProps = {
  hasActiveAccount?: boolean; syncActive?: boolean; initialModeOrder?: readonly string[];
  modeOrderReady?: boolean;
  saveModeOrder?: (order: readonly PlayerQueryMode[]) => Promise<unknown>;
  onOpenOnboarding?: () => void; onOpenMatches?: (handoff: PlayerQueryHandoff) => void;
  onSessionChange?: (sessionId: string | null) => void;
  onAuthenticationInvalid?: () => void;
  externallyAuthenticationInvalid?: boolean;
  queryIntent?: PlayerQueryIntent | null;
};

export function PlayerQueryPage({ hasActiveAccount = false, syncActive = false, initialModeOrder, modeOrderReady = true, saveModeOrder = async () => undefined, onOpenOnboarding, onOpenMatches, onSessionChange, onAuthenticationInvalid, externallyAuthenticationInvalid = false, queryIntent }: PlayerQueryPageProps) {
  const [modeOrder, setModeOrder] = useState(() => normalizeModeOrder(initialModeOrder));
  const [battleTagInput, setBattleTagInput] = useState("");
  const consumedIntentId = useRef<number | null>(null);
  const [modeSaveError, setModeSaveError] = useState<string | null>(null);
  const query = usePlayerQuerySession({ firstMode: modeOrder[0], onSessionChange, onAuthenticationInvalid });
  const persistedModeOrder = normalizeModeOrder(initialModeOrder);
  const modeOrderSynchronized = modeOrderReady && persistedModeOrder.every((mode, index) => mode === modeOrder[index]);
  useEffect(() => setModeOrder(normalizeModeOrder(initialModeOrder)), [initialModeOrder]);
  useEffect(() => {
    query.selectMode(modeOrder[0]);
  }, [modeOrder, query.selectMode]);
  useEffect(() => {
    if (!modeOrderSynchronized || !queryIntent || consumedIntentId.current === queryIntent.id) return;
    consumedIntentId.current = queryIntent.id;
    setBattleTagInput(queryIntent.battleTag);
    void query.startQuery(queryIntent.battleTag);
  }, [modeOrderSynchronized, queryIntent, query.startQuery]);
  const authenticationInvalid = query.state.authenticationInvalid || externallyAuthenticationInvalid;
  const gateMessage = !hasActiveAccount ? "请先登录本地账号后查询玩家" : syncActive ? "当前账号正在同步，请等待同步完成" : null;
  const session = query.state.search.data;
  const open = (match: PlayerQueryMatchRow) => { if (!session || !query.state.matches.data) return; onOpenMatches?.({ sessionId: session.sessionId, battleTag: query.state.battleTag ?? session.battleTag, season: query.state.season, mode: query.state.mode, listScopeKey: query.state.listScopeKey, matches: query.state.matches.data, endReached: query.state.matchesEndReached, selectedMatchId: match.matchId }); };
  const ranks = query.state.ranks.data;

  return <section className="workspace playerQueryWorkspace phase6ReviewWorkspace">
    <div className="playerQueryRegion" data-query-region="identity"><GlassPanel className="playerQueryIdentityPanel" tone="solid">
      <div className="playerQueryIdentityLayout">
        <div className="playerQueryProfileArea"><Resource status={query.state.card.status} error={query.state.card.error} loading="正在加载资料卡…" empty="查询玩家后显示资料卡" retry={() => query.retry("card")}>
          {query.state.card.data ? <PlayerProfileCard card={query.state.card.data} images={query.state.cardImages.data} battleTag={query.state.battleTag ?? query.state.card.data.battleTag} /> : null}
        </Resource>{query.state.card.data && query.state.cardImages.status === "error" ? <button type="button" className="secondaryButton" onClick={() => query.retry("cardImages")}>重试玩家头像</button> : null}</div>
        <div className="playerQuerySearchArea"><PlayerQueryForm value={battleTagInput} onValueChange={setBattleTagInput} disabled={!modeOrderSynchronized || Boolean(gateMessage) || authenticationInvalid} searching={query.state.search.status === "loading"} trailingControl={<PageSeasonSelect label="选择赛季" value={query.state.season} seasons={PLAYER_QUERY_SEASONS} onChange={(season) => { if (season && PLAYER_QUERY_SEASONS.includes(season as PlayerQuerySeason)) query.changeSeason(season as PlayerQuerySeason); }} />} onSubmit={(value) => void query.startQuery(value)} /></div>
      </div>
      {gateMessage ? <div className="playerQueryGate" role="status"><span>{gateMessage}</span>{!hasActiveAccount && onOpenOnboarding ? <button type="button" className="secondaryButton" onClick={onOpenOnboarding}>前往指挥台</button> : null}</div> : null}
      {authenticationInvalid ? <div className="playerQueryGate" role="alert"><span>登录已失效，请在指挥台添加账号以更新登录</span>{onOpenOnboarding ? <button type="button" className="secondaryButton" onClick={onOpenOnboarding}>前往指挥台</button> : null}</div> : null}
      {query.state.search.status === "error" ? <InlineError message={query.state.search.error} /> : null}
    </GlassPanel></div>

    <div className="playerQueryRegion" data-query-region="ranks"><GlassPanel className="playerQueryRanksPanel phase6RoleReport reviewRoleHeroPanel" tone="solid">
      <SectionHeading title="段位总览" />
      <Resource status={query.state.ranks.status} error={query.state.ranks.error} loading="正在加载段位总览…" empty="查询玩家后显示段位总览" retry={() => query.retry("ranks")}>
        {ranks ? <RoleRankOverview competitive={mapRanks(ranks.competitive)} openCompetitive={mapRanks(ranks.openCompetitive ?? [])} stadium={mapRanks(ranks.stadium)} showMissingRoles /> : null}
      </Resource>
    </GlassPanel></div>

    <div className="playerQueryRegion" data-query-region="mode"><GlassPanel className="playerQueryModePanel" tone="solid">
      <div className="playerQueryModeHeader"><SectionHeading title="模式数据" /><PlayerQueryModeTabs order={modeOrder} selected={query.state.mode} disabled={!session || authenticationInvalid} onSelect={query.selectMode} onDropMode={(dragged, target) => { const next = reorderModesForDrop(modeOrder, dragged, target); if (next.every((mode, index) => mode === modeOrder[index])) return; setModeOrder(next); setModeSaveError(null); void saveModeOrder(next).catch(() => setModeSaveError("模式顺序保存失败，本次排序仍会保留到离开页面")); }} /></div>
      {modeSaveError ? <p className="playerQueryInlineError" role="alert">{modeSaveError}</p> : null}
      <div className="playerQueryModeGrid">
        <section className="playerQueryColumnPanel playerQueryMatchesColumn" aria-labelledby="player-query-matches"><SectionHeading id="player-query-matches" title="最近对局" />
          <Resource status={query.state.matches.status} error={query.state.matches.error} loading="正在加载最近对局…" empty="查询玩家后显示最近对局" retry={() => query.retry("matches")}>
            {query.state.matches.data ? <PlayerQueryMatchList matches={query.state.matches.data} onOpenMatch={open} onOpenMore={() => { const first = query.state.matches.data?.[0]; if (first) open(first); }} /> : null}
          </Resource>
        </section>
        <section className="playerQueryColumnPanel playerQueryHeroesColumn" aria-labelledby="player-query-heroes"><SectionHeading id="player-query-heroes" title="常用英雄" />
          <Resource status={query.state.heroes.status} error={query.state.heroes.error} loading="正在加载常用英雄…" empty="查询玩家后显示常用英雄" retry={() => query.retry("heroes")}>
            {query.state.heroes.data ? <PlayerHeroUsage heroes={query.state.heroes.data} /> : null}
          </Resource>
        </section>
      </div>
    </GlassPanel></div>
  </section>;
}

function mapRanks(rows: readonly { role: string; rank: string | null; division: number | null; games: number | null; winRate: number | null; kda: number | null }[]): RoleRankOverviewEntry[] {
  const labels: Record<string, string> = { tank: "重装", damage: "输出", support: "支援", open: "开放" };
  return rows.map((row) => {
    const roleType = row.role === "dps" ? "damage" : row.role === "healer" ? "support" : row.role;
    return { roleType, roleLabel: labels[roleType] ?? row.role, matches: row.games, winRate: row.winRate, kda: row.kda, rankSubTier: row.division, rankKey: row.rank };
  });
}
function SectionHeading({ title, id }: { title: string; id?: string }) { return <header className="playerQuerySectionHeading"><h2 id={id}>{title}</h2></header>; }
function InlineError({ message }: { message: string | null }) { return <p className="playerQueryInlineError" role="alert">{message ?? "请求失败，请稍后重试"}</p>; }
function Resource({ status, error, loading, empty, retry, children }: { status: "idle" | "loading" | "success" | "error"; error: string | null; loading: string; empty: string; retry: () => void; children: React.ReactNode }) {
  if (status === "loading") return <div className="playerQueryResourceState playerQuerySkeleton" role="status" aria-busy="true">{loading}</div>;
  if (status === "error") return <div className="playerQueryResourceState" role="alert"><span>{error ?? "加载失败"}</span><button type="button" className="secondaryButton" onClick={retry}>重试</button></div>;
  if (status === "idle") return <div className="playerQueryResourceState">{empty}</div>;
  return <>{children}</>;
}
