import { listen } from "@tauri-apps/api/event";
import { AlertCircle, Clock3, Database, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { syncSeason } from "../shared/api";
import { GlassPanel, MetricCard, StatusBadge } from "../shared/phase6Components";
import { hasDesktopBackend, productErrorMessage, readableSyncProgress } from "../shared/phase6Shell";
import type { SyncProgress, SyncSummary } from "../shared/types";

export function SyncPage() {
  const [season, setSeason] = useState(22);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  const [summary, setSummary] = useState<SyncSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!hasDesktopBackend()) {
      return;
    }

    let unlisten: (() => void) | undefined;
    void listen<SyncProgress>("sync-progress", (event) => {
      setProgress(event.payload);
    }).then((handler) => {
      unlisten = handler;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  async function runSync() {
    setBusy(true);
    setError(null);
    setSummary(null);
    setProgress({
      season,
      phase: "准备同步",
      processed: 0,
      total: 0,
      currentMatchId: null,
      failedDetails: 0,
      latestErrors: [],
    });
    try {
      setSummary(await syncSeason(season));
    } catch (err) {
      setError(productErrorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="workspace syncWorkspace phase6SyncWorkspace">
      <header className="phase6PageHeader">
        <div>
          <span className="phase6Eyebrow">同步</span>
          <h2>赛季数据同步</h2>
          <p>从已保存的网易大神登录状态读取赛季报告，并写入本地数据库。同步过程只展示产品状态和可恢复问题。</p>
        </div>
        <div className="phase6CommandStatus">
          <StatusBadge tone={busy ? "warn" : "neutral"}>
            <RefreshCw size={15} /> {busy ? "同步中" : "等待同步"}
          </StatusBadge>
          <StatusBadge>
            <Clock3 size={15} /> {readableSyncProgress(progress)}
          </StatusBadge>
        </div>
      </header>

      <GlassPanel className="phase6CommandPanel phase6SyncLauncher" tone="solid">
        <div>
          <span className="phase6Eyebrow">同步控制</span>
          <h3>选择赛季</h3>
          <p>建议先在指挥台确认登录状态，再同步当前赛季。</p>
        </div>
        <div className="phase6SyncControl">
          <label>
            <span>赛季</span>
            <input type="number" min={1} value={season} onChange={(event) => setSeason(Number(event.target.value))} />
          </label>
          <button type="button" className="primaryButton" onClick={runSync} disabled={busy || season <= 0}>
            <RefreshCw size={18} />
            {busy ? "同步中" : "同步赛季"}
          </button>
        </div>
      </GlassPanel>

      {error && (
        <div className="errorBanner phase6RecoverableError">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}

      {progress && (
        <GlassPanel className="phase6ReviewPanel syncSummary" tone="solid">
          <header>
            <span className="phase6Eyebrow">同步进度</span>
            <h3>{progress.phase}</h3>
          </header>
          <div className="phase6MatchMetrics local">
            <MetricCard label="处理进度" value={`${progress.processed}/${progress.total || "-"}`} meta="已处理 / 总数" />
            <MetricCard label="当前比赛" value={progress.currentMatchId ?? "-"} meta="正在读取的比赛编号" />
            <MetricCard label="失败详情" value={String(progress.failedDetails)} meta="可恢复后重试" />
          </div>
          <SafeErrors items={progress.latestErrors} />
        </GlassPanel>
      )}

      {summary && (
        <GlassPanel className="phase6ReviewPanel syncSummary" tone="solid">
          <header>
            <span className="phase6Eyebrow">同步摘要</span>
            <h3>本次写入结果</h3>
          </header>
          <div className="summaryGrid">
            <Metric label="阶段" value={summary.phase} />
            <Metric label="已处理" value={`${summary.processed}/${summary.total}`} />
            <Metric label="导入对局" value={summary.importedMatches} />
            <Metric label="导入详情" value={summary.importedDetails} />
            <Metric label="跳过重复" value={summary.skippedDuplicates} />
            <Metric label="失败详情" value={summary.failedDetails} />
          </div>
          <SafeErrors items={summary.latestErrors} />
        </GlassPanel>
      )}

      {!progress && !summary && !error && (
        <GlassPanel className="phase6ReviewPanel syncSummary" tone="solid">
          <header>
            <span className="phase6Eyebrow">本地数据</span>
            <h3>等待同步任务</h3>
          </header>
          <p className="explainText">
            同步会更新本地比赛、详情和评分缓存。浏览器预览无法连接桌面后端时，会显示可恢复提示；安装版或 Tauri 开发模式下可执行真实同步。
          </p>
          <StatusBadge>
            <Database size={15} /> 本地优先
          </StatusBadge>
        </GlassPanel>
      )}
    </section>
  );
}

function SafeErrors({ items }: { items: string[] }) {
  if (items.length === 0) return null;
  return (
    <div className="safeErrors">
      {items.map((item, index) => (
        <span key={`${item}-${index}`}>{item}</span>
      ))}
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
