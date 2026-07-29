import { useEffect, useState, type FormEvent } from "react";
import { getDeepSeekKeyStatus, saveDeepSeekKey } from "../../shared/api";
import { hasDesktopBackend } from "../../shared/phase6Shell";
import type { DeepSeekKeyStatus } from "../../shared/types";

export function deepSeekStatusLabel(status: DeepSeekKeyStatus) {
  return status.state === "ready" ? "已配置并验证" : "未配置";
}

export function nextDeepSeekKeyValue(currentValue: string, saved: boolean) {
  return saved ? "" : currentValue;
}

export function safeDeepSeekSettingsError(error: unknown) {
  const message = typeof error === "string" ? error : error instanceof Error ? error.message : "";
  if (message.includes("API Key 无效")) {
    return "DeepSeek API Key 无效，请检查后重试";
  }
  if (message.includes("余额不足")) {
    return "DeepSeek 余额不足，请检查账户余额";
  }
  if (message.includes("请求过于频繁")) {
    return "请求过于频繁，请稍后重试";
  }
  if (message.includes("无法连接") || message.includes("服务暂时不可用")) {
    return "暂时无法连接 DeepSeek，请稍后重试";
  }
  return "保存失败，请检查 Key 后重试";
}

export function DeepSeekSettings() {
  const [status, setStatus] = useState<DeepSeekKeyStatus>({
    state: "missing",
    message: "",
  });
  const [apiKey, setApiKey] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);

  useEffect(() => {
    if (!hasDesktopBackend()) {
      return;
    }
    let active = true;
    void getDeepSeekKeyStatus()
      .then((nextStatus) => {
        if (active) {
          setStatus(nextStatus);
        }
      })
      .catch(() => {
        if (active) {
          setFeedback("暂时无法读取配置状态，请稍后重试");
        }
      });
    return () => {
      active = false;
    };
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submitting) {
      return;
    }
    const trimmedApiKey = apiKey.trim();
    if (!trimmedApiKey) {
      setFeedback("请输入 DeepSeek API Key");
      return;
    }

    setSubmitting(true);
    setFeedback(null);
    try {
      const nextStatus = await saveDeepSeekKey(trimmedApiKey);
      setStatus(nextStatus);
      setApiKey(nextDeepSeekKeyValue(apiKey, true));
      setFeedback("已配置并验证");
    } catch (error) {
      setFeedback(safeDeepSeekSettingsError(error));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section className="deepSeekSettings" aria-label="AI 锐评">
      <div className="deepSeekSettingsHeading">
        <span className="phase6Eyebrow">AI 锐评</span>
        <span className="deepSeekSettingsStatus">{deepSeekStatusLabel(status)}</span>
      </div>
      <form className="deepSeekSettingsForm" onSubmit={(event) => void submit(event)}>
        <label htmlFor="deepseek-api-key">DeepSeek API Key</label>
        <div className="deepSeekSettingsControls">
          <input
            className="deepSeekSettingsInput"
            id="deepseek-api-key"
            type="password"
            autoComplete="new-password"
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
            disabled={submitting}
          />
          <button className="deepSeekSettingsSubmit primaryButton" type="submit" disabled={submitting}>
            {submitting ? "验证中…" : "保存并验证"}
          </button>
        </div>
      </form>
      <p className="deepSeekSettingsFeedback" aria-live="polite">
        {feedback ?? ""}
      </p>
    </section>
  );
}
