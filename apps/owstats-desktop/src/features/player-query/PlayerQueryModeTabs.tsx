import { GripVertical } from "lucide-react";
import { useEffect, useState, type PointerEvent } from "react";
import { PLAYER_QUERY_MODE_LABELS, type PlayerQueryMode } from "./playerQueryTypes";

type PlayerQueryModeTabsProps = {
  order: readonly PlayerQueryMode[];
  selected: PlayerQueryMode;
  disabled: boolean;
  onSelect: (mode: PlayerQueryMode) => void;
  onDropMode: (dragged: PlayerQueryMode, target: PlayerQueryMode) => void;
};

export function PlayerQueryModeTabs({ order, selected, disabled, onSelect, onDropMode }: PlayerQueryModeTabsProps) {
  const [dragged, setDragged] = useState<PlayerQueryMode | null>(null);

  useEffect(() => {
    if (!dragged) return;
    const finish = () => setDragged(null);
    window.addEventListener("pointerup", finish, { once: true });
    window.addEventListener("pointercancel", finish, { once: true });
    return () => {
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
    };
  }, [dragged]);

  const beginMouseDrag = (event: PointerEvent, mode: PlayerQueryMode) => {
    if (disabled || event.pointerType !== "mouse" || event.button !== 0) return;
    event.preventDefault();
    setDragged(mode);
  };

  const moveAcross = (event: PointerEvent, target: PlayerQueryMode) => {
    if (!dragged || dragged === target || event.pointerType !== "mouse" || event.buttons !== 1) return;
    onDropMode(dragged, target);
  };

  return <div className="playerQueryModeTabs" role="tablist" aria-label="对局模式顺序">
    {order.map((mode) => <div
      className={`playerQueryModeTabWrap${dragged === mode ? " dragging" : ""}`}
      key={mode}
      onPointerEnter={(event) => moveAcross(event, mode)}
    >
      <button
        className="playerQueryModeDragHandle"
        type="button"
        aria-label={`拖动${PLAYER_QUERY_MODE_LABELS[mode]}调整顺序`}
        disabled={disabled}
        onPointerDown={(event) => beginMouseDrag(event, mode)}
      ><GripVertical size={16} aria-hidden="true" /></button>
      <button type="button" role="tab" aria-selected={selected === mode} disabled={disabled} onClick={() => onSelect(mode)}>{PLAYER_QUERY_MODE_LABELS[mode]}</button>
    </div>)}
  </div>;
}
