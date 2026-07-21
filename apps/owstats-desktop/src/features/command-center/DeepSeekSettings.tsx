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

function safeSettingsError(error: unknown) {
  if (typeof error === "string" && error.trim()) {
    return error;
  }
  if (error instanceof Error && error.message.trim()) {
    return error.message;
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
      setFeedback(safeSettingsError(error));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section className="deepSeekSettings" aria-labelledby="deepseek-settings-title">
      <div className="deepSeekSettingsHeading">
        <div>
          <span className="phase6Eyebrow">AI 锐评</span>
          <h3 id="deepseek-settings-title">DeepSeek API Key</h3>
        </div>
        <span className="deepSeekSettingsStatus">{deepSeekStatusLabel(status)}</span>
      </div>
      <p className="deepSeekSettingsDisclosure">
        仅发送匿名对局事实、评价与回合趋势；不会发送玩家名称、玩家 ID、笔记、原始 JSON 或完整阵容。
      </p>
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
