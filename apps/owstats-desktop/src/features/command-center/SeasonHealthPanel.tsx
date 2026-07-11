import { RefreshCw } from "lucide-react";
import { readableSyncProgress } from "../../shared/phase6Shell";
import type { SeasonDataStatus, SyncProgress } from "../../shared/types";
export function SeasonStatusCard({
  seasonStatus,
  credentialReady,
  syncingSeason,
  progress,
  onSync,
}: {
  seasonStatus: SeasonDataStatus;
  credentialReady: boolean;
  syncingSeason: number | null;
  progress: SyncProgress | null;
  onSync: () => void;
}) {
  const isSyncing = syncingSeason === seasonStatus.season;
  const isSyncBlocked = syncingSeason !== null && !isSyncing;
  const hasData = seasonStatus.matchCount > 0;
  const statusLabel = !credentialReady
    ? "需要登录"
    : isSyncing
        ? "同步中"
        : hasData
          ? "本地数据可用"
          : "无本地数据";

  return (
    <article className="phase8SeasonStatusCard">
      <div className="phase8SeasonStatusHeader">
        <div>
          <span className="phase6Eyebrow">固定赛季</span>
          <h3>{`S${seasonStatus.season}`}</h3>
        </div>
        <span className={credentialReady ? "good" : "warn"}>{statusLabel}</span>
      </div>
      <div className="phase8SeasonStatusStats">
        <DataFact label="本地对局" value={`${seasonStatus.matchCount} 场`} tone={hasData ? "good" : "warn"} />
        <DataFact label="详情数" value={`${seasonStatus.detailCount} 场`} tone={seasonStatus.detailCount > 0 ? "good" : "warn"} />
        <DataFact label="普通对局" value={`${seasonStatus.normalMatchCount} 场`} tone="neutral" />
        <DataFact label="角斗对局" value={`${seasonStatus.arenaMatchCount} 场`} tone="neutral" />
        <DataFact label="待补详情" value={`${seasonStatus.failedDetailCount} 条`} tone={seasonStatus.failedDetailCount > 0 ? "warn" : "good"} />
        <DataFact label="最近同步/导入" value={formatTimestamp(seasonStatus.lastSyncedOrImportedAt)} tone={seasonStatus.lastSyncedOrImportedAt ? "neutral" : "warn"} />
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
            : !credentialReady
              ? "登录后可同步"
              : "当前登录账号可用"}
        </span>
        <button className="primaryButton" type="button" onClick={onSync} disabled={!credentialReady || isSyncBlocked || isSyncing}>
          <RefreshCw size={18} />
          {isSyncing ? "同步中" : `同步 S${seasonStatus.season}`}
        </button>
      </div>
    </article>
  );
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
