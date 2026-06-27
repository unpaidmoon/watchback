import { listen } from "@tauri-apps/api/event";
import { RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { syncSeason } from "../shared/api";
import type { SyncProgress, SyncSummary } from "../shared/types";

export function SyncPage() {
  const [season, setSeason] = useState(22);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  const [summary, setSummary] = useState<SyncSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
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
      latestErrors: []
    });
    try {
      setSummary(await syncSeason(season));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="workspace syncWorkspace">
      <header className="sectionHeader">
        <div>
          <span className="eyebrow">同步</span>
          <h2>赛季同步</h2>
        </div>
        <RefreshCw size={22} />
      </header>

      <div className="syncPanel">
        <label>
          <span>赛季</span>
          <input
            type="number"
            min={1}
            value={season}
            onChange={(event) => setSeason(Number(event.target.value))}
          />
        </label>
        <button type="button" className="primaryButton" onClick={runSync} disabled={busy || season <= 0}>
          <RefreshCw size={18} />
          {busy ? "同步中" : "同步赛季"}
        </button>
      </div>

      {error && <div className="errorBanner">{error}</div>}
      {progress && (
        <section className="playerSection syncSummary">
          <h3>同步进度</h3>
          <div className="summaryGrid">
            <Metric label="当前阶段" value={progress.phase} />
            <Metric label="已处理/总数" value={`${progress.processed}/${progress.total || "-"}`} />
            <Metric label="当前比赛编号" value={progress.currentMatchId ?? "-"} />
            <Metric label="失败数量" value={progress.failedDetails} />
          </div>
          {progress.latestErrors.length > 0 && (
            <div className="safeErrors">
              {progress.latestErrors.map((item, index) => (
                <span key={`${item}-${index}`}>{item}</span>
              ))}
            </div>
          )}
        </section>
      )}
      {summary && (
        <section className="playerSection syncSummary">
          <h3>同步摘要</h3>
          <div className="summaryGrid">
            <Metric label="阶段" value={summary.phase} />
            <Metric label="已处理" value={`${summary.processed}/${summary.total}`} />
            <Metric label="导入对局" value={summary.importedMatches} />
            <Metric label="导入详情" value={summary.importedDetails} />
            <Metric label="跳过重复" value={summary.skippedDuplicates} />
            <Metric label="失败详情" value={summary.failedDetails} />
          </div>
          {summary.latestErrors.length > 0 && (
            <div className="safeErrors">
              {summary.latestErrors.map((item, index) => (
                <span key={`${item}-${index}`}>{item}</span>
              ))}
            </div>
          )}
        </section>
      )}
    </section>
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
