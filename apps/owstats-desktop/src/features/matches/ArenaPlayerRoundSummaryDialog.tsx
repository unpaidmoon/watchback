import { CircleDollarSign, X } from "lucide-react";
import { useEffect, useRef } from "react";
import { resolveArenaModTraitInfo } from "../../shared/assets";
import { HeroAvatar } from "../../shared/phase6Components";
import type { PlayerRow } from "../../shared/types";
import type { ArenaPlayerRoundSummaryRow } from "./arenaPlayerRoundSummary";

export function ArenaPlayerRoundSummaryDialog({
  player,
  rows,
  onClose,
}: {
  player: PlayerRow;
  rows: ArenaPlayerRoundSummaryRow[];
  onClose: () => void;
}) {
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeRef.current?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div
      className="phase7SummaryDialogBackdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section className="phase7SummaryDialog" role="dialog" aria-modal="true" aria-labelledby="phase7-summary-title">
        <header>
          <h2 id="phase7-summary-title">汇总卡片</h2>
          <HeroAvatar heroKey={player.heroGuid || player.heroName} name={player.heroName} size="sm" />
          <span className="phase7SummaryPlayerName">{player.name}</span>
          <button
            ref={closeRef}
            type="button"
            className="phase7SummaryCloseButton"
            onClick={onClose}
            aria-label="关闭汇总卡片"
          >
            <X size={20} aria-hidden="true" />
          </button>
        </header>
        <div className="phase7SummaryDialogBody">
          {rows.map((row) => (
            <div className="phase7SummaryRound" key={row.key}>
              <strong className="phase7SummaryRoundLabel">{row.label}</strong>
              <span className="phase7SummaryWorth">
                <CircleDollarSign size={18} strokeWidth={1.8} aria-hidden="true" />
                <span>{row.startWorth ?? "-"}</span>
              </span>
              <SummarySlots guids={row.traitGuids} count={4} kind="trait" />
              <SummarySlots guids={row.modGuids} count={6} kind="mod" />
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

function SummarySlots({
  guids,
  count,
  kind,
}: {
  guids: string[];
  count: number;
  kind: "trait" | "mod";
}) {
  return (
    <div className={`phase7SummarySlots ${kind}`} aria-label={kind === "trait" ? "异能" : "装备"}>
      {Array.from({ length: count }, (_, index) => {
        const guid = guids[index];
        const info = resolveArenaModTraitInfo(guid);
        const displayName = info?.name ?? guid;
        return (
          <span
            className={kind === "trait" ? "phase7SummaryTraitSlot" : "phase7SummaryModSlot"}
            key={`${kind}-${index}`}
          >
            {guid && info?.iconUrl ? (
              <img src={info.iconUrl} alt={displayName} />
            ) : guid ? (
              <span>{guid}</span>
            ) : null}
          </span>
        );
      })}
    </div>
  );
}
