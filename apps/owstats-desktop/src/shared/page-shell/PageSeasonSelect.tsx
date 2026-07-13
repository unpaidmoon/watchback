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

  return (
    <div className="pageSeasonSelect">
      <label htmlFor={selectId}>{label}</label>
      <select
        id={selectId}
        value={value ?? ""}
        disabled={disabled || seasons.length === 0}
        onChange={(event) => {
          const nextValue = event.currentTarget.value;
          onChange(nextValue === "" ? undefined : Number(nextValue));
        }}
      >
        {includeAll ? <option value="">{allLabel}</option> : null}
        {seasons.map((season) => (
          <option key={season} value={season}>
            S{season}
          </option>
        ))}
      </select>
    </div>
  );
}
