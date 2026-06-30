import { listen } from "@tauri-apps/api/event";
import { AlertCircle, CheckCircle2, Clock3, Database, LogIn, RefreshCw, ShieldCheck, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { cancelDashenLogin, getOnboardingStatus, startDashenLogin, syncSeason } from "../shared/api";
import { GlassPanel, MetricCard, StatusBadge } from "../shared/phase6Components";
import { commandCenterMetrics, credentialLabel, credentialTone, hasDesktopBackend, productErrorMessage, readableSyncProgress } from "../shared/phase6Shell";
import type { CredentialLoginProgress, OnboardingStatus, SyncProgress } from "../shared/types";

export function OnboardingPage() {
  const navigate = useNavigate();
  const [season, setSeason] = useState(22);
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [syncProgress, setSyncProgress] = useState<SyncProgress | null>(null);
  const [loginProgress, setLoginProgress] = useState<CredentialLoginProgress | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [loggingIn, setLoggingIn] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!hasDesktopBackend()) {
      return;
    }

    let unlistenSync: (() => void) | undefined;
    let unlistenLogin: (() => void) | undefined;
    void listen<SyncProgress>("sync-progress", (event) => setSyncProgress(event.payload)).then((handler) => {
      unlistenSync = handler;
    });
    void listen<CredentialLoginProgress>("credential-login-progress", (event) => setLoginProgress(event.payload)).then((handler) => {
      unlistenLogin = handler;
    });
    return () => {
      unlistenSync?.();
      unlistenLogin?.();
    };
  }, []);

  useEffect(() => {
    void loadStatus(season);
  }, [season]);

  async function loadStatus(nextSeason: number) {
    setError(null);
    try {
      setStatus(await getOnboardingStatus(nextSeason));
    } catch (err) {
      setError(productErrorMessage(err));
    }
  }

  async function runLogin() {
    setLoggingIn(true);
    setError(null);
    try {
      const result = await startDashenLogin();
      setStatus((current) => (current ? { ...current, credentialStatus: result.status } : current));
      await loadStatus(season);
    } catch (err) {
      setError(productErrorMessage(err));
      await loadStatus(season);
    } finally {
      setLoggingIn(false);
    }
  }

  async function cancelLogin() {
    setError(null);
    try {
      await cancelDashenLogin();
      setLoginProgress({ phase: "cancelled", message: "已取消登录等待。" });
    } catch (err) {
      setError(productErrorMessage(err));
    } finally {
      setLoggingIn(false);
    }
  }

  async function runSync() {
    setSyncing(true);
    setError(null);
    try {
      await syncSeason(season);
      await loadStatus(season);
    } catch (err) {
      setError(productErrorMessage(err));
    } finally {
      setSyncing(false);
    }
  }

  const credentialReady = status?.credentialStatus.state === "saved";
  const metrics = useMemo(() => commandCenterMetrics(status), [status]);
  const warnings = status?.health.warnings ?? [];

  return (
    <section className="workspace onboardingWorkspace phase6CommandCenter">
      <header className="phase6CommandCenterHero">
        <div>
          <span className="phase6Eyebrow">赛季指挥台</span>
          <h2>登录、同步与本地数据状态</h2>
          <p>确认账号状态，刷新赛季数据，然后进入对局、复盘、排行和分析模型。</p>
        </div>
        <div className="phase6CommandStatus">
          <StatusBadge tone={credentialTone(status?.credentialStatus.state)}>
            <ShieldCheck size={15} /> {credentialLabel(status?.credentialStatus.state)}
          </StatusBadge>
          <StatusBadge tone={status?.canEnterReview ? "good" : "warn"}>
            <Database size={15} /> {status?.canEnterReview ? "数据可用" : "等待数据"}
          </StatusBadge>
          <StatusBadge>
            <Clock3 size={15} /> {status?.localData.lastSyncedOrImportedAt ?? "暂无同步时间"}
          </StatusBadge>
        </div>
      </header>

      {error && (
        <div className="errorBanner inline phase6RecoverableError">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}

      <div className="phase6CommandMetrics">
        {metrics.map((metric) => (
          <MetricCard key={metric.label} label={metric.label} value={metric.value} meta={metric.meta} />
        ))}
      </div>

      <div className="phase6CommandGridReal">
        <GlassPanel className="phase6CommandPanel" tone="solid">
          <div>
            <span className="phase6Eyebrow">账号</span>
            <h3>网易大神登录</h3>
            <p>{status?.credentialStatus.message ?? "正在读取登录状态..."}</p>
          </div>
          <div className="phase6StatusLine">
            <CheckCircle2 size={18} />
            <span>{credentialReady ? `已保存账号角色：${status?.credentialStatus.roleId ?? "-"}` : "不会显示 token、cookie 或任何敏感凭据。"}</span>
          </div>
          {loginProgress && <p className="phase6ProgressText">登录进度：{loginProgress.message}</p>}
          <div className="buttonRow">
            <button className="primaryButton" type="button" onClick={runLogin} disabled={loggingIn}>
              <LogIn size={18} />
              {credentialReady ? "重新登录" : "登录网易大神"}
            </button>
            {loggingIn && (
              <button className="ghostButton" type="button" onClick={cancelLogin}>
                <X size={18} />
                取消
              </button>
            )}
          </div>
        </GlassPanel>

        <GlassPanel className="phase6CommandPanel" tone="solid">
          <div>
            <span className="phase6Eyebrow">同步</span>
            <h3>赛季数据刷新</h3>
            <p>{readableSyncProgress(syncProgress)}</p>
          </div>
          <div className="phase6SyncControl">
            <label>
              <span>赛季</span>
              <input type="number" min={1} value={season} onChange={(event) => setSeason(Number(event.target.value))} />
            </label>
            <button className="primaryButton" type="button" onClick={runSync} disabled={syncing || season <= 0 || !credentialReady}>
              <RefreshCw size={18} />
              {syncing ? "同步中" : "同步赛季"}
            </button>
          </div>
          {!credentialReady && <p className="phase6ProgressText">请先完成登录。已有本地数据时，仍可进入下方分析页面。</p>}
        </GlassPanel>

        <GlassPanel className="phase6CommandPanel phase6CommandPanelWide" tone="solid">
          <div>
            <span className="phase6Eyebrow">本地数据</span>
            <h3>可用数据状态</h3>
            <p>本地数据库是所有页面的数据来源；这里仅展示产品可理解的状态和可恢复问题。</p>
          </div>
          <div className="phase6DataSummary">
            <Metric label="数据来源" value={status?.localData.dataSource ?? "-"} />
            <Metric label="赛季" value={status?.localData.season ? `S${status.localData.season}` : "-"} />
            <Metric label="新鲜度" value={status?.localData.freshnessRisk ?? "-"} />
          </div>
          {warnings.length ? (
            <div className="safeErrors">
              {warnings.map((warning) => (
                <span key={warning}>{warning}</span>
              ))}
            </div>
          ) : (
            <p className="phase6ProgressText">没有需要处理的数据问题。</p>
          )}
        </GlassPanel>
      </div>

      <div className="phase6CommandActions">
        <button type="button" className="ghostButton" onClick={() => navigate(`/matches?season=${season}`)} disabled={!status?.canEnterReview}>
          查看对局
        </button>
        <button type="button" className="ghostButton" onClick={() => navigate("/ranking")} disabled={!status?.canEnterReview}>
          查看排行
        </button>
        <button type="button" className="primaryButton" onClick={() => navigate(`/review?season=${season}`)} disabled={!status?.canEnterReview}>
          <Database size={18} />
          进入个人复盘
        </button>
      </div>
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
