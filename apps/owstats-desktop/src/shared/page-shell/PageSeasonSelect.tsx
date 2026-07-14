import { useId } from "react";

export type PageSeasonSelectProps = {
  value: number | undefined;
  seasons: readonly number[];
  onChange: (season: number | undefined) => void;
  disabled?: boolean;
  label?: string;
  includeAll?: boolean;
  allLabel?: string;
};

export function PageSeasonSelect({
  value,
  seasons,
  onChange,
  disabled = false,
  label = "赛季",
  includeAll = false,
  allLabel = "全部赛季",
}: PageSeasonSelectProps) {
  const selectId = useId();
  const hasSelectedSeason = value !== undefined && seasons.includes(value);
  const selectValue = value === undefined ? "" : String(value);

  return (
    <div className="pageSeasonSelect">
      <label htmlFor={selectId}>{label}</label>
      <select
        id={selectId}
        value={selectValue}
        disabled={disabled || seasons.length === 0}
        onChange={(event) => {
          const nextValue = event.currentTarget.value;
          if (nextValue === "") {
            if (includeAll) {
              onChange(undefined);
            }
            return;
          }

          const nextSeason = Number(nextValue);
          if (seasons.includes(nextSeason)) {
            onChange(nextSeason);
          }
        }}
      >
        {includeAll ? (
          <option value="">{allLabel}</option>
        ) : value === undefined ? (
          <option value="" disabled>
            请选择赛季
          </option>
        ) : null}
        {value !== undefined && !hasSelectedSeason ? (
          <option value={value} disabled>
            S{value} 不可用
          </option>
        ) : null}
        {seasons.map((season) => (
          <option key={season} value={season}>
            S{season}
          </option>
        ))}
      </select>
    </div>
  );
}
