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
  if (accountOperationBusy) {
    return { statusLabel: "账号处理中", syncHint: "账号操作完成后可同步" };
  }
  if (!credentialReady) {
    return { statusLabel: "需要登录", syncHint: "登录后可同步" };
  }
  if (!targetSyncAllowed) {
    return { statusLabel: "目标待确认", syncHint: "确认当前目标后可同步" };
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
