import { RefreshCw } from "lucide-react";
import { readableSyncProgress } from "../../shared/phase6Shell";
import type { SeasonDataStatus, SyncMode, SyncProgress } from "../../shared/types";
import { formatCompactTimestamp, seasonSyncGateCopy } from "./SeasonHealthPanel";

export type SeasonSyncWorkbenchProps = {
  selectedSeason: number;
  currentSeason: number;
  syncableSeasons: number[];
  seasonStatuses: SeasonDataStatus[];
  credentialReady: boolean;
  targetSyncAllowed: boolean;
  accountOperationBusy: boolean;
  syncingSeason: number | null;
  syncingMode: SyncMode | null;
  progress: SyncProgress | null;
  syncError: string | null;
  syncNotice: string | null;
  onSelectSeason: (season: number) => void;
  onFullFetch: (season: number) => void;
  onQuickUpdate: (season: number) => void;
};

export function resolveCommandCenterSeason(
  requested: number,
  syncableSeasons: number[],
  currentSeason: number,
) {
  return syncableSeasons.includes(requested) ? requested : currentSeason;
}

export function SeasonSyncWorkbench({
  selectedSeason,
  currentSeason,
  syncableSeasons,
  seasonStatuses,
  credentialReady,
  targetSyncAllowed,
  accountOperationBusy,
  syncingSeason,
  syncingMode,
  progress,
  syncError,
  syncNotice,
  onSelectSeason,
  onFullFetch,
  onQuickUpdate,
}: SeasonSyncWorkbenchProps) {
  const sortedStatuses = [...seasonStatuses].sort((left, right) => right.season - left.season);
  const isSyncing = syncingSeason === selectedSeason;
  const isSelectedFullFetch = isSyncing && syncingMode === "fullFetch";
  const selectedHasData = seasonStatuses.some((item) => item.season === selectedSeason && item.matchCount > 0);
  const gateCopy = seasonSyncGateCopy(
    credentialReady,
    targetSyncAllowed,
    accountOperationBusy,
    isSyncing,
    selectedHasData,
  );
  const firstSeason = syncableSeasons[syncableSeasons.length - 1] ?? currentSeason;

  return (
    <section className="seasonSyncWorkbench" aria-label="赛季同步工作台">
      <section className="seasonSyncControl" aria-labelledby="season-sync-control-title">
        <div>
          <span className="phase6Eyebrow">赛季同步</span>
          <label id="season-sync-control-title" htmlFor="command-center-season-select">同步一个赛季</label>
        </div>
        <select
          id="command-center-season-select"
          aria-label="选择同步赛季"
          value={selectedSeason}
          onChange={(event) => onSelectSeason(Number(event.target.value))}
          disabled={syncingSeason !== null || syncableSeasons.length === 0}
        >
          {syncableSeasons.map((season) => (
            <option key={season} value={season}>
              {season === currentSeason ? `S${season} · 当前赛季` : `S${season}`}
            </option>
          ))}
        </select>
        <p className="seasonSyncRange">{`可同步范围：S${firstSeason}–S${currentSeason} · 当前赛季 S${currentSeason}`}</p>
        <button
          className="primaryButton seasonSyncButton"
          type="button"
          onClick={() => onFullFetch(selectedSeason)}
          disabled={!credentialReady || !targetSyncAllowed || accountOperationBusy || syncingSeason !== null}
        >
          <RefreshCw size={18} />
          {isSelectedFullFetch ? "全量抓取中…" : "全量抓取"}
        </button>
        <div className="seasonSyncFeedback" aria-live={syncError ? "assertive" : "polite"}>
          {syncError ? (
            <span className="error">{syncError}</span>
          ) : syncNotice ? (
            <span className="good">{syncNotice}</span>
          ) : (
            <span>{isSyncing ? readableSyncProgress(progress) : gateCopy.syncHint}</span>
          )}
        </div>
      </section>

      <section className="syncedSeasonLedger" aria-labelledby="synced-season-ledger-title">
        <div className="syncedSeasonLedgerHeading">
          <h3 id="synced-season-ledger-title">已同步数据</h3>
          <span>按赛季从新到旧</span>
        </div>
        {sortedStatuses.length === 0 ? (
          <p className="syncedSeasonLedgerEmpty">暂无已同步赛季，请从左侧选择赛季开始同步。</p>
        ) : (
          <div className="syncedSeasonLedgerTable" role="table" aria-label="已同步赛季数据">
            <div className="syncedSeasonLedgerHeader" role="row">
              <span role="columnheader">赛季</span>
              <span role="columnheader">详情覆盖度</span>
              <span role="columnheader">本地对局</span>
              <span role="columnheader">最近同步</span>
              <span role="columnheader">操作</span>
            </div>
            {sortedStatuses.map((seasonStatus) => {
              const rowSyncing = syncingSeason === seasonStatus.season;
              return (
                <div
                  key={seasonStatus.season}
                  className={`syncedSeasonLedgerRow${selectedSeason === seasonStatus.season ? " selected" : ""}${rowSyncing ? " syncing" : ""}`}
                  role="row"
                >
                  <div className="syncedSeasonSelectCell" role="cell">
                    <button
                      className="syncedSeasonSelect"
                      type="button"
                      aria-label={`选择 S${seasonStatus.season}`}
                      aria-pressed={selectedSeason === seasonStatus.season}
                      onClick={() => onSelectSeason(seasonStatus.season)}
                    >
                      <span className="syncedSeasonName">{`S${seasonStatus.season}`}{rowSyncing ? <small>同步中</small> : null}</span>
                    </button>
                  </div>
                  <div className="syncedSeasonCoverageCell" role="cell">
                    <span className="syncedSeasonCoverage">
                      <span className="syncedSeasonCoverageTrack" aria-hidden="true">
                        <span style={{ width: `${seasonStatus.coveragePercent}%` }} />
                      </span>
                      <span>{seasonStatus.coveragePercent}%</span>
                    </span>
                  </div>
                  <div className="syncedSeasonMatchCountCell" role="cell">
                    {seasonStatus.matchCount} 场
                  </div>
                  <div className="syncedSeasonLastSyncCell" role="cell">
                    {formatCompactTimestamp(seasonStatus.lastSyncedOrImportedAt)}
                  </div>
                  <div className="syncedSeasonUpdateCell" role="cell">
                    <button
                      className="ghostButton syncedSeasonUpdateButton"
                      type="button"
                      aria-label={`更新 S${seasonStatus.season}`}
                      onClick={() => onQuickUpdate(seasonStatus.season)}
                      disabled={!credentialReady || !targetSyncAllowed || accountOperationBusy || syncingSeason !== null}
                    >
                      {rowSyncing && syncingMode === "quickUpdate" ? "更新中…" : "更新"}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>
    </section>
  );
}
