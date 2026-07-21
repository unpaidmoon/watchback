import { useEffect, useRef, useState } from "react";
import {
  generateMatchAiRoast,
  getDeepSeekKeyStatus,
} from "../../shared/api";
import type { AiMatchRoastView } from "../../shared/types";

export const AI_ROAST_LOADING_PHRASES = [
  "正在翻阅本场数据…",
  "正在寻找关键证据…",
  "正在确认谁在带飞…",
  "正在核对锅位归属…",
  "正在申请本场\u5c01号…",
  "正在组织锐评措辞…",
] as const;

export const AI_ROAST_LOADING_INTERVAL_MS = 2_000;
export const AI_ROAST_LIVE_MESSAGE = "正在生成 AI 锐评";

type AiRoastError = {
  message: string;
  recoverable: boolean;
};

type AiMatchRoastProps = {
  matchId: string;
  initialRoast: AiMatchRoastView | null;
  detailReady: boolean;
};

type MatchScopedValue<T> = {
  matchId: string;
  value: T;
};

export function clearBusyMatch(
  currentBusyMatchId: string | null,
  requestMatchId: string,
) {
  return currentBusyMatchId === requestMatchId ? null : currentBusyMatchId;
}

function errorMessage(error: unknown) {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "锐评生成失败，请稍后再试";
}

export function classifyAiRoastError(error: unknown): AiRoastError {
  const message = errorMessage(error);

  if (message.includes("API Key 无效")) {
    return {
      message: "DeepSeek API Key 无效，请在指挥台重新配置",
      recoverable: false,
    };
  }
  if (message.includes("余额不足")) {
    return {
      message: "DeepSeek 余额不足，请前往 DeepSeek 检查余额",
      recoverable: false,
    };
  }
  if (message.includes("尚未配置") || message.includes("API Key 未配置")) {
    return {
      message: "请先在指挥台配置 DeepSeek API Key",
      recoverable: false,
    };
  }
  if (message.includes("生成超时")) {
    return { message: "锐评生成超时，请重试", recoverable: true };
  }
  if (message.includes("安全素材")) {
    return { message: "锐评输出格式异常，请重试", recoverable: true };
  }

  const recoverable = [
    "\u5df2\u6709\u4e00\u573a AI \u9510\u8bc4\u6b63\u5728\u751f\u6210",
    "请求过于频繁",
    "服务暂时繁忙",
    "无法连接 DeepSeek 服务",
  ].some((fragment) => message.includes(fragment));

  return { message, recoverable };
}

export function AiMatchRoast({
  matchId,
  initialRoast,
  detailReady,
}: AiMatchRoastProps) {
  const [generatedRoast, setGeneratedRoast] =
    useState<MatchScopedValue<AiMatchRoastView> | null>(null);
  const [error, setError] = useState<MatchScopedValue<AiRoastError> | null>(null);
  const [busyMatchId, setBusyMatchId] = useState<string | null>(null);
  const [loadingPhraseIndex, setLoadingPhraseIndex] = useState(0);
  const requestTokenRef = useRef(0);
  const matchIdRef = useRef(matchId);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  matchIdRef.current = matchId;

  useEffect(() => {
    return () => {
      requestTokenRef.current += 1;
      if (intervalRef.current !== null) {
        clearInterval(intervalRef.current);
      }
    };
  }, []);

  const roast =
    generatedRoast?.matchId === matchId ? generatedRoast.value : initialRoast;
  const currentError = error?.matchId === matchId ? error.value : null;
  const busy = busyMatchId === matchId;

  async function generateRoast() {
    if (!detailReady || busy || roast) {
      return;
    }

    const requestToken = requestTokenRef.current + 1;
    requestTokenRef.current = requestToken;
    const requestMatchId = matchIdRef.current;
    setError(null);
    setBusyMatchId(requestMatchId);
    setLoadingPhraseIndex(0);

    const loadingInterval = setInterval(() => {
      setLoadingPhraseIndex(
        (current) => (current + 1) % AI_ROAST_LOADING_PHRASES.length,
      );
    }, AI_ROAST_LOADING_INTERVAL_MS);
    intervalRef.current = loadingInterval;

    try {
      const keyStatus = await getDeepSeekKeyStatus();
      if (
        requestTokenRef.current !== requestToken ||
        requestMatchId !== matchIdRef.current
      ) {
        return;
      }
      if (keyStatus.state !== "ready") {
        setError({
          matchId: requestMatchId,
          value: classifyAiRoastError(keyStatus.message),
        });
        return;
      }

      const result = await generateMatchAiRoast(requestMatchId);
      if (
        requestTokenRef.current !== requestToken ||
        requestMatchId !== matchIdRef.current
      ) {
        return;
      }
      setGeneratedRoast({ matchId: requestMatchId, value: result.aiRoast });
      setError(null);
    } catch (requestError) {
      if (
        requestTokenRef.current !== requestToken ||
        requestMatchId !== matchIdRef.current
      ) {
        return;
      }
      setError({
        matchId: requestMatchId,
        value: classifyAiRoastError(requestError),
      });
    } finally {
      clearInterval(loadingInterval);
      if (intervalRef.current === loadingInterval) {
        intervalRef.current = null;
      }
      setBusyMatchId((current) => clearBusyMatch(current, requestMatchId));
    }
  }

  return (
    <section className="aiMatchRoast" aria-label="AI 锐评">
      <span className="visuallyHidden" aria-live="polite">
        {busy ? AI_ROAST_LIVE_MESSAGE : ""}
      </span>

      {roast ? (
        <div className="aiMatchRoastResult">
          <h3>{roast.award}</h3>
          <p>{roast.commentary}</p>
        </div>
      ) : (
        <div className="aiMatchRoastAction">
          {busy ? (
            <p aria-hidden="true">
              {AI_ROAST_LOADING_PHRASES[loadingPhraseIndex]}
            </p>
          ) : null}
          {currentError ? (
            <p className="aiMatchRoastError" role="alert">
              {currentError.message}
            </p>
          ) : null}
          {currentError && currentError.recoverable && !busy ? (
            <button type="button" onClick={() => void generateRoast()}>重试</button>
          ) : !currentError ? (
            <button
              type="button"
              disabled={!detailReady || busy}
              onClick={() => void generateRoast()}
            >
              {detailReady ? "生成 AI 锐评" : "对局详情完整后可生成锐评"}
            </button>
          ) : null}
        </div>
      )}
    </section>
  );
}
