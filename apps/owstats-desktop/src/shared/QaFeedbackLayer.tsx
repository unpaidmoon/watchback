import { Check, Clipboard, MessageSquare, X } from "lucide-react";
import { createContext, type ReactNode, useContext, useEffect, useMemo, useState } from "react";
import { useLocation } from "react-router-dom";
import {
  formatQaFeedbackText,
  isQaEnabledFromSearch,
  qaPageForPath,
  qaRegionsForPath,
  type QaRegionDefinition,
} from "./qaFeedback";

type QaFeedbackContextValue = {
  enabled: boolean;
  pageLabel: string;
  regions: QaRegionDefinition[];
  copyRegion: (region: QaRegionDefinition) => Promise<void>;
};

const QaFeedbackContext = createContext<QaFeedbackContextValue | null>(null);

type QaFeedbackProviderProps = {
  children: ReactNode;
};

type QaRegionProps = {
  id: string;
  children: ReactNode;
  className?: string;
};

export function QaFeedbackProvider({ children }: QaFeedbackProviderProps) {
  const location = useLocation();
  const [enabled, setEnabled] = useState(() => isQaEnabledFromSearch(location.search));
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const page = useMemo(() => qaPageForPath(location.pathname), [location.pathname]);
  const regions = useMemo(() => qaRegionsForPath(location.pathname), [location.pathname]);

  useEffect(() => {
    setEnabled(isQaEnabledFromSearch(location.search));
  }, [location.search]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.altKey && event.key.toLowerCase() === "q") {
        event.preventDefault();
        setEnabled((value) => !value);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const value = useMemo<QaFeedbackContextValue>(
    () => ({
      enabled,
      pageLabel: page.label,
      regions,
      async copyRegion(region) {
        const text = formatQaFeedbackText({ pageLabel: page.label, regionId: region.id, regionLabel: region.label });
        await navigator.clipboard?.writeText(text);
        setCopiedId(region.id);
        window.setTimeout(() => setCopiedId(null), 1400);
      },
    }),
    [enabled, page.label, regions],
  );

  return (
    <QaFeedbackContext.Provider value={value}>
      {children}
      {enabled && <QaFeedbackPanel copiedId={copiedId} />}
    </QaFeedbackContext.Provider>
  );
}

export function QaRegion({ id, children, className = "" }: QaRegionProps) {
  const qa = useContext(QaFeedbackContext);
  const region = qa?.regions.find((item) => item.id === id) ?? { id, label: id };

  return (
    <div className={`phase6QaRegion ${className}`} data-qa-region={id}>
      {qa?.enabled && (
        <button type="button" className="phase6QaBadge" title="复制反馈模板" onClick={() => void qa.copyRegion(region)}>
          {id}
        </button>
      )}
      {children}
    </div>
  );
}

function QaFeedbackPanel({ copiedId }: { copiedId: string | null }) {
  const qa = useContext(QaFeedbackContext);
  if (!qa) return null;

  return (
    <aside className="phase6QaPanel" aria-label="UI 反馈标注">
      <header>
        <span>
          <MessageSquare size={15} /> UI 反馈模式
        </span>
        <small>Alt+Q 隐藏</small>
      </header>
      <p>点击区域编号复制反馈模板。</p>
      <div>
        {qa.regions.map((region) => (
          <button type="button" key={region.id} onClick={() => void qa.copyRegion(region)}>
            <span>{region.id}</span>
            <small>{region.label}</small>
            {copiedId === region.id ? <Check size={14} /> : <Clipboard size={14} />}
          </button>
        ))}
      </div>
      <button type="button" className="phase6QaCloseHint" onClick={() => window.dispatchEvent(new KeyboardEvent("keydown", { altKey: true, key: "q" }))}>
        <X size={14} /> 隐藏标注
      </button>
    </aside>
  );
}
