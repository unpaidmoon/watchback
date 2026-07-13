import { X } from "lucide-react";
import type { ActiveMatchFilterChip, VisibleMatchFilterKey } from "./matchFilters";

export function ActiveFilterChips({ chips, onRemove }: {
  chips: ActiveMatchFilterChip[];
  onRemove: (key: VisibleMatchFilterKey) => void;
}) {
  if (chips.length === 0) return null;
  return (
    <div className="activeFilterChips" aria-label="有效筛选">
      {chips.map((chip) => (
        <button type="button" key={chip.key} onClick={() => onRemove(chip.key)} aria-label={`移除${chip.label}`}>
          <span>{chip.label}</span>
          <X size={13} aria-hidden="true" />
        </button>
      ))}
    </div>
  );
}
