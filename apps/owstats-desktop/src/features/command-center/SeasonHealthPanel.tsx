import { RefreshCw } from "lucide-react";
import { readableSyncProgress } from "../../shared/phase6Shell";
import type { SeasonDataStatus, SyncProgress } from "../../shared/types";
export function SeasonStatusCard({
  seasonStatus,
  credentialReady,
  targetSyncAllowed,
  accountOperationBusy,
  syncingSeason,
  progress,
  onSync,
}: {
  seasonStatus: SeasonDataStatus;
  credentialReady: boolean;
  targetSyncAllowed: boolean;
  accountOperationBusy: boolean;
  syncingSeason: number | null;
  progress: SyncProgress | null;
  onSync: () => void;
}) {
  const isSyncing = syncingSeason === seasonStatus.season;
  const isSyncBlocked = syncingSeason !== null && !isSyncing;
  const hasData = seasonStatus.matchCount > 0;
  const syncGateReady = credentialReady && targetSyncAllowed && !accountOperationBusy;
  const gateCopy = seasonSyncGateCopy(credentialReady, targetSyncAllowed, accountOperationBusy, isSyncing, hasData);

  return (
    <article className="phase8SeasonStatusCard">
      <div className="phase8SeasonStatusHeader">
        <div>
          <span className="phase6Eyebrow">当前赛季</span>
          <h3>{`S${seasonStatus.season}`}</h3>
        </div>
        <span className={isSyncing || syncGateReady ? "good" : "warn"}>{gateCopy.statusLabel}</span>
      </div>
      <div className="phase8SeasonStatusStats">
        <DataFact label="本地对局" value={`${seasonStatus.matchCount} 场`} tone={hasData ? "good" : "warn"} />
        <DataFact label="详情数" value={`${seasonStatus.detailCount} 场`} tone={seasonStatus.detailCount > 0 ? "good" : "warn"} />
        <DataFact label="普通对局" value={`${seasonStatus.normalMatchCount} 场`} tone="neutral" />
        <DataFact label="角斗对局" value={`${seasonStatus.arenaMatchCount} 场`} tone="neutral" />
        <DataFact label="待补详情" value={`${seasonStatus.failedDetailCount} 条`} tone={seasonStatus.failedDetailCount > 0 ? "warn" : "good"} />
        <DataFact label="最近同步/导入" value={formatCompactTimestamp(seasonStatus.lastSyncedOrImportedAt)} tone={seasonStatus.lastSyncedOrImportedAt ? "neutral" : "warn"} />
      </div>
      <div className="phase8SeasonCoverage">
        <div>
          <span>详情覆盖率</span>
          <strong>{seasonStatus.coveragePercent}%</strong>
        </div>
        <div className="phase6CoverageTrack" aria-hidden="true">
          <span style={{ width: `${seasonStatus.coveragePercent}%` }} />
        </div>
      </div>
      <div className="phase8SeasonSyncFooter">
        <span>
          {isSyncing
            ? readableSyncProgress(progress)
            : gateCopy.syncHint}
        </span>
        <button className="primaryButton" type="button" onClick={onSync} disabled={!credentialReady || !targetSyncAllowed || accountOperationBusy || isSyncBlocked || isSyncing}>
          <RefreshCw size={18} />
          {isSyncing ? "同步中" : `同步 S${seasonStatus.season}`}
        </button>
      </div>
    </article>
  );
}

export function seasonSyncGateCopy(
  credentialReady: boolean,
  targetSyncAllowed: boolean,
  accountOperationBusy: boolean,
  isSyncing: boolean,
  hasData: boolean,
) {
  if (isSyncing) {
    return { statusLabel: "同步中", syncHint: "同步中" };
  }
  if (!credentialReady) {
    return { statusLabel: "需要登录", syncHint: "登录后可同步" };
  }
  if (!targetSyncAllowed) {
    return { statusLabel: "目标待确认", syncHint: "确认当前目标后可同步" };
  }
  if (accountOperationBusy) {
    return { statusLabel: "账号处理中", syncHint: "账号操作完成后可同步" };
  }
  return {
    statusLabel: hasData ? "本地数据可用" : "无本地数据",
    syncHint: "当前目标可同步",
  };
}

export function DataFact({ label, value, tone }: { label: string; value: string; tone: "good" | "warn" | "neutral" }) {
  return (
    <span className={tone}>
      <span>{label}</span>
      <strong>{value}</strong>
    </span>
  );
}

export function formatTimestamp(value: string | null | undefined) {
  if (!value) {
    return "暂无同步";
  }

  const compact = value.replace(/(\.\d{3})\d+/, "$1");
  const parsed = new Date(compact);
  if (Number.isNaN(parsed.getTime())) {
    return value.replace("T", " ").replace(/(\.\d+)?([+-]\d\d:?\d\d|Z)?$/, "");
  }

  const pad = (part: number) => String(part).padStart(2, "0");
  return `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(parsed.getDate())} ${pad(parsed.getHours())}:${pad(parsed.getMinutes())}`;
}

export function formatCompactTimestamp(value: string | null | undefined) {
  const formatted = formatTimestamp(value);
  return /^\d{4}-\d{2}-\d{2}\s\d{2}:\d{2}$/.test(formatted) ? formatted.slice(5) : formatted;
}
