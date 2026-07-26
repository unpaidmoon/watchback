import { ArrowLeft } from "lucide-react";
import { useEffect } from "react";
import { PlayerQueryMatchesWorkbench, type PlayerQueryMatchesWorkbenchApi } from "../features/player-query/PlayerQueryMatchesWorkbench";
import { loadPlayerQueryMatchDetail, loadPlayerQueryModePage, releasePlayerQueryMoreData } from "../features/player-query/playerQueryApi";
import { PLAYER_QUERY_MODE_LABELS, type PlayerQueryMode, type PlayerQuerySeason } from "../features/player-query/playerQueryTypes";
import type { PlayerQueryHandoff } from "./PlayerQueryPage";
import { PageHeader } from "../shared/page-shell";
import "../styles/features/player-query-matches.css";

const defaultApi: PlayerQueryMatchesWorkbenchApi = {
  loadPage: loadPlayerQueryModePage,
  loadDetail: loadPlayerQueryMatchDetail,
  releaseMoreData: releasePlayerQueryMoreData,
};

export type PlayerQueryMatchesPageProps = {
  handoff?: PlayerQueryHandoff | null;
  api?: PlayerQueryMatchesWorkbenchApi;
  onBack: () => void;
  onSessionMissing?: () => void;
  onAuthenticationInvalid?: () => void;
  onQueryPlayer?: (battleTag: string) => void;
};

export function PlayerQueryMatchesPage({ handoff, api = defaultApi, onBack, onSessionMissing, onAuthenticationInvalid, onQueryPlayer }: PlayerQueryMatchesPageProps) {
  useEffect(() => { if (!handoff) onSessionMissing?.(); }, [handoff, onSessionMissing]);
  if (!handoff) {
    return <section className="workspace playerQueryMatchesWorkspace"><div className="emptyState">查询会话不存在，正在返回查询首页。</div></section>;
  }
  const season = handoff.season as PlayerQuerySeason;
  const mode = handoff.mode as PlayerQueryMode;
  return <section className="workspace playerQueryMatchesWorkspace">
    <PageHeader
      className="playerQueryMatchesHeader"
      title={`${handoff.battleTag} · S${season} · ${PLAYER_QUERY_MODE_LABELS[mode]}`}
      description=""
      actions={<button type="button" className="secondaryButton playerQueryMatchesBack" onClick={onBack}><ArrowLeft size={17} aria-hidden="true" />返回查询</button>}
    />
    <PlayerQueryMatchesWorkbench
      sessionId={handoff.sessionId}
      season={season}
      mode={mode}
      listScopeKey={handoff.listScopeKey}
      initialMatches={handoff.matches}
      initialEndReached={(handoff as PlayerQueryHandoff & { endReached?: boolean }).endReached}
      initialSelectedMatchId={handoff.selectedMatchId || handoff.matches[0]?.matchId || ""}
      api={api}
      onAuthenticationInvalid={onAuthenticationInvalid}
      onQueryPlayer={onQueryPlayer}
    />
  </section>;
}
