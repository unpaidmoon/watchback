import { RefreshCw } from "lucide-react";
import { readableSyncProgress } from "../../shared/phase6Shell";
import { backgroundStatusLabel } from "../../shared/seasonSyncEvents";
import type { SeasonBackgroundStatus, SeasonDataStatus, SyncMode, SyncProgress } from "../../shared/types";
import { formatCompactTimestamp, seasonSyncGateCopy } from "./SeasonHealthPanel";

export type SeasonSyncWorkbenchProps = {
  selectedSeason: number;
  minimumSyncSeason: number;
  seasonStatuses: SeasonDataStatus[];
  credentialReady: boolean;
  targetSyncAllowed: boolean;
  accountOperationBusy: boolean;
  syncingSeason: number | null;
  syncingMode: SyncMode | null;
  progress: SyncProgress | null;
  syncError: string | null;
  syncNotice: string | null;
  autoUpdateEnabled: boolean;
  autoUpdateSaving: boolean;
  backgroundStatuses: Record<number, SeasonBackgroundStatus>;
  onSelectSeason: (season: number) => void;
  onFullFetch: (season: number) => void;
  onQuickUpdate: (season: number) => void;
  onRetryBackground: (season: number) => void;
  onAutoUpdateChange: (enabled: boolean) => void;
};

export function resolveCommandCenterSeason(
  requested: number,
  minimumSyncSeason: number,
  defaultSeason: number,
) {
  return requested >= minimumSyncSeason ? requested : defaultSeason;
}

export function decrementTargetSeason(season: number, minimumSyncSeason: number) {
  return Math.max(minimumSyncSeason, season - 1);
}

export function incrementTargetSeason(season: number) {
  return season + 1;
}

export function runSeasonQuickUpdate(
  season: number,
  onQuickUpdate: (season: number) => void,
) {
  onQuickUpdate(season);
}

export function SeasonSyncWorkbench({
  selectedSeason,
  minimumSyncSeason,
  seasonStatuses,
  credentialReady,
  targetSyncAllowed,
  accountOperationBusy,
  syncingSeason,
  syncingMode,
  progress,
  syncError,
  syncNotice,
  autoUpdateEnabled,
  autoUpdateSaving,
  backgroundStatuses,
  onSelectSeason,
  onFullFetch,
  onQuickUpdate,
  onRetryBackground,
  onAutoUpdateChange,
}: SeasonSyncWorkbenchProps) {
  const sortedStatuses = [...seasonStatuses].sort((left, right) => right.season - left.season);
  const isSyncing = syncingSeason === selectedSeason;
  const isSelectedFullFetch = isSyncing && syncingMode === "fullFetch";
  const selectedBackground = backgroundStatuses[selectedSeason];
  const selectedBackgroundBusy = selectedBackground?.status === "queued" || selectedBackground?.status === "running";
  const selectedHasData = seasonStatuses.some((item) => item.season === selectedSeason && item.matchCount > 0);
  const gateCopy = seasonSyncGateCopy(
    credentialReady,
    targetSyncAllowed,
    accountOperationBusy,
    isSyncing,
    selectedHasData,
  );
  return (
    <section className="seasonSyncWorkbench" aria-label="赛季同步工作台">
      <section className="seasonSyncControl" aria-labelledby="season-sync-control-title">
        <div>
          <span className="phase6Eyebrow">赛季同步</span>
          <span id="season-sync-control-title" className="seasonSyncControlLabel">目标赛季</span>
        </div>
        <label className="seasonAutoUpdateToggle">
          <span>
            <strong>自动更新</strong>
            <small>启动或切换账号时更新最新本地赛季</small>
          </span>
          <input
            type="checkbox"
            checked={autoUpdateEnabled}
            disabled={autoUpdateSaving}
            onChange={(event) => onAutoUpdateChange(event.target.checked)}
          />
        </label>
        <div className="seasonTargetStepper" aria-label="选择目标赛季">
          <button
            type="button"
            aria-label="减少目标赛季"
            onClick={() => onSelectSeason(decrementTargetSeason(selectedSeason, minimumSyncSeason))}
            disabled={syncingSeason !== null || selectedSeason <= minimumSyncSeason}
          >
            −
          </button>
          <output aria-live="polite">{`S${selectedSeason}`}</output>
          <button
            type="button"
            aria-label="增加目标赛季"
            onClick={() => onSelectSeason(incrementTargetSeason(selectedSeason))}
            disabled={syncingSeason !== null}
          >
            +
          </button>
        </div>
        <p className="seasonSyncRange">{`最低支持 S${minimumSyncSeason}`}</p>
        <button
          className="primaryButton seasonSyncButton"
          type="button"
          onClick={() => onFullFetch(selectedSeason)}
          disabled={!credentialReady || !targetSyncAllowed || accountOperationBusy || syncingSeason !== null || selectedBackgroundBusy}
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
              const background = backgroundStatuses[seasonStatus.season];
              const backgroundBusy = background?.status === "queued" || background?.status === "running";
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
                    {background ? (
                      <small className={background.status === "failed" ? "error" : "syncedSeasonBackgroundStatus"}>
                        {backgroundStatusLabel(background)}
                      </small>
                    ) : null}
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
                      onClick={() => background?.retryable
                        ? onRetryBackground(seasonStatus.season)
                        : runSeasonQuickUpdate(seasonStatus.season, onQuickUpdate)}
                      disabled={!credentialReady || !targetSyncAllowed || accountOperationBusy || syncingSeason !== null || backgroundBusy}
                    >
                      {background?.retryable ? "重试" : rowSyncing && syncingMode === "quickUpdate" ? "更新中…" : backgroundBusy ? "后台处理中" : "更新"}
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
