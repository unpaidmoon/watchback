import { useEffect, useRef, useState } from "react";
import {
  generateMatchAiRoast,
  getDeepSeekKeyStatus,
} from "../../shared/api";
import type { AiMatchRoastView, AiRoastInsight } from "../../shared/types";

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

function RoastInsightEntry({ insight }: { insight: AiRoastInsight }) {
  return (
    <article className="aiMatchRoastEntry">
      <h4>{insight.title}</h4>
      <p>{insight.commentary}</p>
    </article>
  );
}

function RoastReport({ roast }: { roast: AiMatchRoastView }) {
  const report = roast.report;
  if (!report) {
    return (
      <div className="aiMatchRoastNarrative">
        <h3>{roast.award}</h3>
        <p>{roast.commentary}</p>
      </div>
    );
  }

  return (
    <div className="aiMatchRoastReport">
      <section className="aiMatchRoastSection">
        <h3>本场判词</h3>
        <article className="aiMatchRoastEntry">
          <h4>{report.verdict.title}</h4>
          <p>{report.verdict.summary}</p>
        </article>
      </section>
      <section className="aiMatchRoastSection">
        <h3>胜负手</h3>
        {report.decisiveFactors.map((insight, index) => (
          <RoastInsightEntry key={`${index}-${insight.title}`} insight={insight} />
        ))}
      </section>
      <section className="aiMatchRoastSection">
        <h3>你这局</h3>
        <RoastInsightEntry insight={report.currentPlayer} />
      </section>
      <section className="aiMatchRoastSection">
        <h3>队友与对位</h3>
        {report.teamAndMatchups.map((insight, index) => (
          <RoastInsightEntry key={`${index}-${insight.title}`} insight={insight} />
        ))}
      </section>
      <section className="aiMatchRoastSection">
        <h3>终审锐评</h3>
        <p className="aiMatchRoastClosing">{report.closingRoast}</p>
      </section>
    </div>
  );
}

export function clearBusyMatch(
  currentBusyMatchId: string | null,
  requestMatchId: string,
) {
  return currentBusyMatchId === requestMatchId ? null : currentBusyMatchId;
}

export function registerLoadingInterval<T>(
  loadingIntervals: ReadonlySet<T>,
  loadingInterval: T,
) {
  return new Set(loadingIntervals).add(loadingInterval);
}

export function unregisterLoadingInterval<T>(
  loadingIntervals: ReadonlySet<T>,
  loadingInterval: T,
) {
  const remainingIntervals = new Set(loadingIntervals);
  remainingIntervals.delete(loadingInterval);
  return remainingIntervals;
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
  if (message.includes("长度上限") || message.includes("超出长度限制")) {
    return { message: "锐评输出未完整生成，请重试", recoverable: true };
  }
  if (message.includes("安全规则拦截")) {
    return { message: "锐评内容被安全规则拦截，请重试", recoverable: true };
  }
  if (message.includes("安全素材") || message.includes("锐评格式无效")) {
    return { message: "锐评输出格式异常，请重试", recoverable: true };
  }
  if (message.includes("可核验数据")) {
    return {
      message: "这场对局缺少可核验数据，暂时无法生成锐评",
      recoverable: false,
    };
  }
  if (message.includes("详情尚未完整同步")) {
    return {
      message: "对局详情尚未完整同步，请先更新对局数据",
      recoverable: false,
    };
  }
  if (message.includes("锐评保存失败")) {
    return {
      message: "锐评保存失败，请检查数据目录后重试",
      recoverable: true,
    };
  }

  const recoverable = [
    "\u5df2\u6709\u4e00\u573a AI \u9510\u8bc4\u6b63\u5728\u751f\u6210",
    "请求过于频繁",
    "服务暂时繁忙",
    "无法连接 DeepSeek 服务",
  ].some((fragment) => message.includes(fragment));

  if (recoverable) {
    return { message: "DeepSeek 暂时不可用，请稍后重试", recoverable: true };
  }
  return { message: "锐评生成失败，请稍后重试", recoverable: false };
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
  const mountedRef = useRef(true);
  const loadingIntervalsRef = useRef(
    new Set<ReturnType<typeof setInterval>>(),
  );
  matchIdRef.current = matchId;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestTokenRef.current += 1;
      for (const loadingInterval of loadingIntervalsRef.current) {
        clearInterval(loadingInterval);
      }
      loadingIntervalsRef.current.clear();
    };
  }, []);

  const roast =
    generatedRoast?.matchId === matchId ? generatedRoast.value : initialRoast;
  const currentError = error?.matchId === matchId ? error.value : null;
  const busy = busyMatchId === matchId;

  async function generateRoast(regenerate = false) {
    if (!detailReady || busy || (roast && !regenerate)) {
      return;
    }

    const requestToken = requestTokenRef.current + 1;
    requestTokenRef.current = requestToken;
    const requestMatchId = matchIdRef.current;
    setError(null);
    setBusyMatchId(requestMatchId);
    setLoadingPhraseIndex(0);

    const loadingInterval = setInterval(() => {
      if (!mountedRef.current) {
        return;
      }
      setLoadingPhraseIndex(
        (current) => (current + 1) % AI_ROAST_LOADING_PHRASES.length,
      );
    }, AI_ROAST_LOADING_INTERVAL_MS);
    loadingIntervalsRef.current = registerLoadingInterval(
      loadingIntervalsRef.current,
      loadingInterval,
    );

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

      const result = await generateMatchAiRoast(requestMatchId, regenerate);
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
      loadingIntervalsRef.current = unregisterLoadingInterval(
        loadingIntervalsRef.current,
        loadingInterval,
      );
      if (mountedRef.current) {
        setBusyMatchId((current) => clearBusyMatch(current, requestMatchId));
      }
    }
  }

  return (
    <section className="aiMatchRoast" aria-label="AI 锐评">
      <span className="visuallyHidden" aria-live="polite">
        {busy ? AI_ROAST_LIVE_MESSAGE : ""}
      </span>

      {roast ? (
        <div className="aiMatchRoastResult">
          <RoastReport roast={roast} />
          {currentError ? (
            <p className="aiMatchRoastError" role="alert">
              {currentError.message}
            </p>
          ) : null}
          <button
            type="button"
            disabled={!detailReady || busy}
            onClick={() => void generateRoast(true)}
          >
            {busy ? "正在重新生成…" : "重新生成"}
          </button>
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
