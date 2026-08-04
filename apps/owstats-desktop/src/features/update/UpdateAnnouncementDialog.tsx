import { CloudDownload, Github, X } from "lucide-react";
import { useEffect, useRef, useState, type RefObject } from "react";
import { openUpdateDownload } from "../../shared/api";
import { productErrorMessage } from "../../shared/phase6Shell";
import type { UpdateAnnouncement, UpdateDownloadSource } from "../../shared/types";

type FocusTarget = {
  focus(): void;
};

type DialogKeyEvent = {
  key: string;
  shiftKey?: boolean;
  preventDefault(): void;
};

export function handleUpdateDialogKeyDown(
  event: DialogKeyEvent,
  focusableElements: readonly FocusTarget[],
  activeElement: FocusTarget | null,
  onDismiss: () => void,
) {
  if (event.key === "Escape") {
    event.preventDefault();
    onDismiss();
    return;
  }
  if (event.key !== "Tab" || focusableElements.length === 0) return;
  const first = focusableElements[0];
  const last = focusableElements[focusableElements.length - 1];
  if (event.shiftKey && activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

export function restoreUpdateDialogFocus(target: (FocusTarget & { isConnected: boolean }) | null) {
  if (target?.isConnected) target.focus();
}

const sourceLabels: Record<UpdateDownloadSource, string> = {
  github: "GitHub",
  baidu: "百度网盘",
  quark: "夸克网盘",
};

export type UpdateAnnouncementDialogViewProps = {
  announcement: UpdateAnnouncement;
  currentVersion: string;
  openingSources: ReadonlySet<UpdateDownloadSource>;
  error: string | null;
  rootRef: RefObject<HTMLElement>;
  dismissButtonRef: RefObject<HTMLButtonElement>;
  onDismiss(): void;
  onOpenDownload(source: UpdateDownloadSource): void;
};

export function UpdateAnnouncementDialogView({
  announcement,
  currentVersion,
  openingSources,
  error,
  rootRef,
  dismissButtonRef,
  onDismiss,
  onOpenDownload,
}: UpdateAnnouncementDialogViewProps) {
  return (
    <div className="updateAnnouncementBackdrop" role="presentation" onMouseDown={onDismiss}>
      <section
        ref={rootRef}
        className="updateAnnouncementDialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="update-announcement-title"
        aria-describedby="update-announcement-description"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="updateAnnouncementHeader">
          <div className="updateAnnouncementIcon" aria-hidden="true">
            <CloudDownload size={22} />
          </div>
          <div className="updateAnnouncementHeading">
            <span className="updateAnnouncementEyebrow">发现新版本</span>
            <h2 id="update-announcement-title">{announcement.title}</h2>
          </div>
          <button className="iconButton updateAnnouncementClose" type="button" aria-label="关闭更新公告" onClick={onDismiss}>
            <X aria-hidden="true" size={20} />
          </button>
        </header>

        <p id="update-announcement-description" className="updateAnnouncementMeta">
          <span>当前版本 v{currentVersion}</span>
          <span aria-hidden="true">·</span>
          <strong>最新版本 v{announcement.latestVersion}</strong>
          <span aria-hidden="true">·</span>
          <span>发布于 {announcement.publishedAt}</span>
        </p>

        <div className="updateAnnouncementContent">
          <h3>更新内容</h3>
          <ul>
            {announcement.notes.map((note, index) => <li key={`${index}-${note}`}>{note}</li>)}
          </ul>
        </div>

        {error ? <p className="updateAnnouncementError" role="alert">{error}</p> : null}

        <footer className="updateAnnouncementActions">
          <div className="updateAnnouncementDownloads" aria-label="下载渠道">
            {(["github", "baidu", "quark"] as const).map((source) => {
              const opening = openingSources.has(source);
              return (
                <button
                  key={source}
                  className={source === "github" ? "updateDownloadButton updateDownloadButton-github" : "updateDownloadButton"}
                  type="button"
                  data-source={source}
                  disabled={opening}
                  aria-busy={opening || undefined}
                  onClick={() => onOpenDownload(source)}
                >
                  {source === "github" ? <Github aria-hidden="true" size={17} /> : <CloudDownload aria-hidden="true" size={17} />}
                  {opening ? `正在打开${sourceLabels[source]}…` : sourceLabels[source]}
                </button>
              );
            })}
          </div>
          <button ref={dismissButtonRef} className="ghostButton updateAnnouncementLater" type="button" onClick={onDismiss}>
            稍后更新
          </button>
        </footer>
      </section>
    </div>
  );
}

export type UpdateAnnouncementDialogProps = {
  announcement: UpdateAnnouncement;
  currentVersion: string;
  onDismiss(): void;
  openDownload?: (source: UpdateDownloadSource) => Promise<void>;
};

export function UpdateAnnouncementDialog({
  announcement,
  currentVersion,
  onDismiss,
  openDownload = openUpdateDownload,
}: UpdateAnnouncementDialogProps) {
  const rootRef = useRef<HTMLElement>(null);
  const dismissButtonRef = useRef<HTMLButtonElement>(null);
  const [openingSources, setOpeningSources] = useState<ReadonlySet<UpdateDownloadSource>>(new Set());
  const openingSourcesRef = useRef(new Set<UpdateDownloadSource>());
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dismissButtonRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      const focusable = Array.from(
        rootRef.current?.querySelectorAll<HTMLElement>("button:not(:disabled), [href], [tabindex]:not([tabindex='-1'])") ?? [],
      );
      handleUpdateDialogKeyDown(
        event,
        focusable,
        document.activeElement instanceof HTMLElement ? document.activeElement : null,
        onDismiss,
      );
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      restoreUpdateDialogFocus(previousFocus);
    };
  }, [onDismiss]);

  async function handleOpenDownload(source: UpdateDownloadSource) {
    if (openingSourcesRef.current.has(source)) return;
    openingSourcesRef.current.add(source);
    setError(null);
    setOpeningSources((current) => new Set(current).add(source));
    try {
      await openDownload(source);
    } catch (cause) {
      setError(`打开${sourceLabels[source]}失败：${productErrorMessage(cause)}`);
    } finally {
      openingSourcesRef.current.delete(source);
      setOpeningSources((current) => {
        const next = new Set(current);
        next.delete(source);
        return next;
      });
    }
  }

  return (
    <UpdateAnnouncementDialogView
      announcement={announcement}
      currentVersion={currentVersion}
      openingSources={openingSources}
      error={error}
      rootRef={rootRef}
      dismissButtonRef={dismissButtonRef}
      onDismiss={onDismiss}
      onOpenDownload={(source) => void handleOpenDownload(source)}
    />
  );
}
