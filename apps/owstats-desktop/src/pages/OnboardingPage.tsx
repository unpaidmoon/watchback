import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, Database, LogIn, RefreshCw, ShieldCheck, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { cancelDashenLogin, getOnboardingStatus, startDashenLogin, syncSeason } from "../shared/api";
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
      setError(String(err));
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
      setError(String(err));
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
      setError(String(err));
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
      setError(String(err));
    } finally {
      setSyncing(false);
    }
  }

  const credentialReady = status?.credentialStatus.state === "saved";

  return (
    <section className="workspace onboardingWorkspace">
      <header className="sectionHeader">
        <div>
          <span className="eyebrow">首次设置</span>
          <h2>登录、同步与本地数据</h2>
        </div>
        <ShieldCheck size={22} />
      </header>

      {error && <div className="errorBanner inline">{error}</div>}

      <div className="wizardGrid">
        <section className="playerSection">
          <h3>1. 网易大神登录</h3>
          <p className="explainText">{status?.credentialStatus.message ?? "正在读取凭证状态..."}</p>
          <div className="statusLine">
            <CheckCircle2 size={18} />
            <span>
              {credentialReady ? `已保存：role_id ${status?.credentialStatus.roleId ?? "-"}` : "未显示 token、cookie、customerToken 或原始凭证内容。"}
            </span>
          </div>
          {status?.credentialStatus.savedAt && <p className="explainText">保存时间：{status.credentialStatus.savedAt}</p>}
          {loginProgress && <p className="explainText">登录进度：{loginProgress.message}</p>}
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
        </section>

        <section className="playerSection">
          <h3>2. 赛季同步</h3>
          <div className="syncPanel compactPanel">
            <label>
              <span>赛季</span>
              <input type="number" min={1} value={season} onChange={(event) => setSeason(Number(event.target.value))} />
            </label>
            <button className="primaryButton" type="button" onClick={runSync} disabled={syncing || season <= 0 || !credentialReady}>
              <RefreshCw size={18} />
              {syncing ? "同步中" : "同步赛季"}
            </button>
          </div>
          {!credentialReady && <p className="explainText">请先登录网易大神，再同步赛季数据。已有本地数据仍可进入功能页。</p>}
        </section>

        <section className="playerSection">
          <h3>3. 本地数据</h3>
          <div className="summaryGrid">
            <Metric label="来源" value={status?.localData.dataSource ?? "-"} />
            <Metric label="预设/开放" value={status?.localData.presetMatches ?? 0} />
            <Metric label="详情" value={status?.localData.detailMatches ?? 0} />
            <Metric label="最后更新" value={status?.localData.lastSyncedOrImportedAt ?? "-"} />
            <Metric label="新鲜度风险" value={status?.localData.freshnessRisk ?? "-"} />
          </div>
        </section>

        <section className="playerSection">
          <h3>4. 数据健康</h3>
          {status?.health.warnings.length ? (
            <div className="safeErrors">
              {status.health.warnings.map((warning) => (
                <span key={warning}>{warning}</span>
              ))}
            </div>
          ) : (
            <p className="explainText">本地赛季数据可读。健康提示不作为进入复盘的硬门禁。</p>
          )}
          {syncProgress && (
            <div className="summaryGrid topGap">
              <Metric label="阶段" value={syncProgress.phase} />
              <Metric label="进度" value={`${syncProgress.processed}/${syncProgress.total || "-"}`} />
              <Metric label="失败详情" value={syncProgress.failedDetails} />
            </div>
          )}
        </section>
      </div>

      <div className="reviewActions">
        <button type="button" className="ghostButton" onClick={() => navigate(`/matches?season=${season}`)} disabled={!status?.canEnterReview}>
          查看对局
        </button>
        <button type="button" className="primaryButton" onClick={() => navigate(`/review?season=${season}`)} disabled={!status?.canEnterReview}>
          <Database size={18} />
          进入个人赛季复盘
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
