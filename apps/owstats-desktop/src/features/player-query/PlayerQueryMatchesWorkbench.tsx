import { useCallback, useEffect, useRef, useState } from "react";
import { MatchDetailContent } from "../matches/MatchDetailContent";
import { MatchListCard } from "../matches/MatchListCard";
import type { MatchDetailView } from "../../shared/types";
import { decodePlayerQueryError, loadPlayerQueryMatchDetail } from "./playerQueryApi";
import { PLAYER_QUERY_MODE_LABELS, type PlayerQueryListScopeKey, type PlayerQueryMatchDetail, type PlayerQueryMatchRow, type PlayerQueryMode, type PlayerQuerySeason } from "./playerQueryTypes";
import { isAuthenticationError } from "./playerQueryState";
import { usePlayerQueryPagination, type PlayerQueryPaginationApi } from "./usePlayerQueryPagination";

export type PlayerQueryMatchesWorkbenchApi = PlayerQueryPaginationApi & {
  loadDetail: typeof loadPlayerQueryMatchDetail;
};

export type PlayerQueryMatchesWorkbenchProps = {
  sessionId: string;
  season: PlayerQuerySeason;
  mode: PlayerQueryMode;
  listScopeKey: PlayerQueryListScopeKey;
  initialMatches: readonly PlayerQueryMatchRow[];
  initialEndReached?: boolean;
  initialSelectedMatchId: string;
  api: PlayerQueryMatchesWorkbenchApi;
  onAuthenticationInvalid?: () => void;
  onQueryPlayer?: (battleTag: string) => void;
};

export function displayPlayerQueryDetail(detail: PlayerQueryMatchDetail, row?: PlayerQueryMatchRow, mode?: PlayerQueryMode): MatchDetailView {
  const safePlayers = (players: PlayerQueryMatchDetail["teammates"] | null | undefined) => (players ?? []).map((player) => ({
    ...player,
    modGuids: player.modGuids ?? [],
    traitGuids: player.traitGuids ?? [],
    perks: player.perks ?? [],
    heroStats: player.heroStats ?? [],
  }));
  const safeSlice = (slice: NonNullable<PlayerQueryMatchDetail["arenaDetail"]>["summary"]) => ({
    ...slice,
    teammates: safePlayers(slice.teammates),
    enemies: safePlayers(slice.enemies),
  });
  return {
    ...detail,
    gameMode: mode ?? detail.gameMode ?? null,
    modeLabel: mode ? PLAYER_QUERY_MODE_LABELS[mode] : detail.modeLabel,
    mapGuid: detail.mapGuid ?? row?.mapKey ?? null,
    mapName: detail.mapName ?? row?.mapName ?? null,
    heroGuid: detail.heroGuid ?? row?.heroKey ?? null,
    heroName: detail.heroName ?? row?.heroName ?? null,
    teammates: safePlayers(detail.teammates),
    enemies: safePlayers(detail.enemies),
    arenaDetail: detail.arenaDetail ? {
      summary: safeSlice(detail.arenaDetail.summary),
      rounds: (detail.arenaDetail.rounds ?? []).map(safeSlice),
    } : null,
    teamAwards: detail.teamAwards ?? [],
    rawJson: null,
  };
}

export function PlayerQueryMatchesWorkbench({
  sessionId,
  season,
  mode,
  listScopeKey,
  initialMatches,
  initialEndReached,
  initialSelectedMatchId,
  api,
  onAuthenticationInvalid,
  onQueryPlayer,
}: PlayerQueryMatchesWorkbenchProps) {
  const pagination = usePlayerQueryPagination({ sessionId, season, mode, listScopeKey, initialMatches, initialEndReached, api, onAuthenticationInvalid });
  const fallbackId = initialMatches[0]?.matchId ?? "";
  const [selectedMatchId, setSelectedMatchId] = useState(initialSelectedMatchId || fallbackId);
  const [detail, setDetail] = useState<PlayerQueryMatchDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const detailGeneration = useRef(0);
  const initialDetailTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const selectedRow = pagination.matches.find((match) => match.matchId === selectedMatchId);

  const requestDetail = useCallback(async (matchId: string) => {
    const row = pagination.matches.find((match) => match.matchId === matchId);
    if (!row) return;
    const generation = ++detailGeneration.current;
    setDetail(null);
    setDetailError(null);
    setDetailLoading(true);
    try {
      const next = await api.loadDetail(sessionId, row.matchKind, row.matchId);
      if (generation === detailGeneration.current) setDetail(next);
    } catch (cause) {
      if (generation !== detailGeneration.current) return;
      const message = decodePlayerQueryError(cause, "读取对局详情失败").message;
      if (isAuthenticationError(cause)) onAuthenticationInvalid?.();
      else setDetailError(message);
    } finally {
      if (generation === detailGeneration.current) setDetailLoading(false);
    }
  }, [api, onAuthenticationInvalid, pagination.matches, sessionId]);

  useEffect(() => {
    // Defer the entry request so StrictMode's synthetic first mount cannot issue a
    // duplicate upstream detail request.
    if (initialDetailTimer.current) clearTimeout(initialDetailTimer.current);
    initialDetailTimer.current = selectedMatchId ? setTimeout(() => {
      initialDetailTimer.current = null;
      void requestDetail(selectedMatchId);
    }, 0) : null;
    return () => {
      if (initialDetailTimer.current) clearTimeout(initialDetailTimer.current);
      initialDetailTimer.current = null;
      detailGeneration.current += 1;
    };
    // Entry selection is deliberately loaded once; later changes are explicit row clicks.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const selectMatch = useCallback((matchId: string) => {
    setSelectedMatchId(matchId);
    void requestDetail(matchId);
  }, [requestDetail]);

  return <div className="playerQueryMatchesWorkbench matchesWorkbench detailOpen">
    <aside
      className="matchListPane phase6WorkbenchList playerQueryMatchesList"
      aria-label="查询对局列表"
      onScroll={(event) => pagination.onListScroll(event.currentTarget)}
    >
      <div className="phase6Timeline playerQueryMatchesTimeline">
        {pagination.matches.map((match) => <MatchListCard key={match.matchId} variant="playerQuery" match={match} selected={match.matchId === selectedMatchId} onSelect={selectMatch} />)}
        <div className="playerQueryPaginationSentinel" aria-hidden="true" />
        {pagination.loading ? <div className="playerQueryPaginationStatus" role="status">正在加载更多对局…</div> : null}
        {pagination.error ? <div className="playerQueryPaginationError" role="alert"><span>{pagination.error}</span><button type="button" className="secondaryButton" onClick={pagination.retry}>重试</button></div> : null}
        {pagination.endReached ? <div className="playerQueryPaginationStatus">已加载全部对局</div> : null}
      </div>
    </aside>
    <aside className="matchDetailPane phase6WorkbenchDetail playerQueryMatchesDetail" aria-label="查询对局详情">
      <MatchDetailContent detail={detail ? displayPlayerQueryDetail(detail, selectedRow, mode) : null} loading={detailLoading} error={detailError} onRetry={() => { if (selectedMatchId) void requestDetail(selectedMatchId); }} onQueryPlayer={onQueryPlayer} />
    </aside>
  </div>;
}
