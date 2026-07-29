import { ChevronRight, RefreshCw, UserRound, X } from "lucide-react";
import { useEffect, useRef } from "react";
import type { PlayerGameAction } from "../../shared/types";

export type ProfileSummary = {
  name: string;
  title: string | null;
  titleIconUrl: string | null;
  avatarUrl: string | null;
  endorsementLevel: number | null;
  playTimeHours: number | null;
  gameAction: PlayerGameAction | null;
};

type AvatarAction = {
  onClick: () => void;
  busy: boolean;
  disabled: boolean;
};

export function ProfileSummaryCard({
  profile,
  avatarAction,
  nameHeadingId,
  showTitleIcon = true,
  gameActionLoading = false,
  onGameActionClick,
}: {
  profile: ProfileSummary;
  avatarAction?: AvatarAction;
  nameHeadingId?: string;
  showTitleIcon?: boolean;
  gameActionLoading?: boolean;
  onGameActionClick?: () => void;
}) {
  const avatar = profile.avatarUrl
    ? <img src={profile.avatarUrl} alt="" width={128} height={128} />
    : <UserRound aria-hidden="true" size={36} />;
  const canOpenGameAction = Boolean(onGameActionClick);

  return (
    <article className="profileSummaryCard">
      {showTitleIcon && profile.titleIconUrl ? (
        <div className="commandCenterTitleIcon" aria-hidden="true">
          <img src={profile.titleIconUrl} alt="" width={480} height={96} draggable={false} />
        </div>
      ) : null}
      <div className="commandCenterIdentity">
        {avatarAction ? (
          <button
            className="phase6ProfileAvatar commandCenterProfileRefresh"
            type="button"
            title={avatarAction.busy ? "正在刷新账号资料" : "点击头像刷新账号资料"}
            aria-label={avatarAction.busy ? "正在刷新账号资料" : "点击头像刷新账号资料"}
            aria-busy={avatarAction.busy}
            onClick={avatarAction.onClick}
            disabled={avatarAction.disabled}
          >
            {avatar}
            {avatarAction.busy ? <span className="commandCenterProfileRefreshState"><RefreshCw aria-hidden="true" size={22} className="isSpinning" /></span> : null}
          </button>
        ) : (
          <div className="phase6ProfileAvatar">{avatar}</div>
        )}
        <div className="commandCenterIdentityCopy">
          <div className="commandCenterIdentityLine">
            <h2 id={nameHeadingId}>{profile.name}</h2>
            <span className="commandCenterProfileTitle">{profile.title ?? "暂无头衔"}</span>
          </div>
          <div className="commandCenterAccountFacts">
            <ProfileInlineStat label="赞赏等级：" value={profile.endorsementLevel ?? "-"} />
            <ProfileInlineStat label="游戏时间：" value={profile.playTimeHours != null ? `${profile.playTimeHours.toFixed(1)} 小时` : "-"} />
            <button
              className="commandCenterGameActionTrigger"
              type="button"
              aria-haspopup="dialog"
              aria-busy={gameActionLoading}
              onClick={onGameActionClick}
              disabled={!canOpenGameAction || gameActionLoading}
            >
              <span>本周表现：</span>
              <strong>{gameActionLoading ? "加载中…" : profile.gameAction?.comment ?? (onGameActionClick ? "点击查看" : "暂无数据")}</strong>
              <ChevronRight aria-hidden="true" size={16} />
            </button>
          </div>
        </div>
      </div>
    </article>
  );
}

function ProfileInlineStat({ label, value }: { label: string; value: string | number }) {
  return <span className="commandCenterInlineStat"><span>{label}</span><strong>{value}</strong></span>;
}

export function ProfileGameActionDialog({ action, onClose }: { action: PlayerGameAction; onClose: () => void }) {
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeButtonRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      } else if (event.key === "Tab") {
        event.preventDefault();
        closeButtonRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      if (previousFocus?.isConnected) previousFocus.focus();
    };
  }, [onClose]);

  return (
    <div className="gameActionDialogBackdrop" role="presentation" onMouseDown={onClose}>
      <section className="gameActionDialog" role="dialog" aria-modal="true" aria-labelledby="game-action-dialog-title" aria-describedby="game-action-dialog-comment" onMouseDown={(event) => event.stopPropagation()}>
        <header>
          <div><span className="phase6Eyebrow">玩家互动概览</span><h3 id="game-action-dialog-title">本周表现</h3></div>
          <button ref={closeButtonRef} className="iconButton gameActionDialogClose" type="button" aria-label="关闭本周表现弹窗" onClick={onClose}><X aria-hidden="true" size={20} /></button>
        </header>
        <p id="game-action-dialog-comment" className="gameActionDialogComment">{action.comment ?? "暂无数据"}</p>
        <dl className="gameActionDialogStats">
          <GameActionStat label="称赞他人" value={action.sendHonorsCnt} />
          <GameActionStat label="收到称赞" value={action.getHonorsCnt} />
          <GameActionStat label="被举报次数" value={action.reportedCnt} warning={(action.reportedCnt ?? 0) > 0} />
        </dl>
      </section>
    </div>
  );
}

function GameActionStat({ label, value, warning = false }: { label: string; value: number | null; warning?: boolean }) {
  return <div className={warning ? "isWarning" : undefined}><dt>{label}</dt><dd>{value ?? "-"}</dd></div>;
}
