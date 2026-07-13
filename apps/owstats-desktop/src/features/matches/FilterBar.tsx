import { useEffect, useState } from "react";
import { searchMatchPlayerOptions } from "../../shared/api";
import type { FilterOptions, MatchFilters, PlayerSearchOption } from "../../shared/types";
import { ActiveFilterChips } from "./ActiveFilterChips";
import { AdvancedMatchFilters } from "./AdvancedMatchFilters";
import { MatchFilterToolbar } from "./MatchFilterToolbar";
import {
  activeMatchFilterChips,
  clearVisibleMatchFilters,
  countAdvancedMatchFilters,
  removeMatchFilter,
} from "./matchFilters";

type Props = {
  filters: MatchFilters;
  options: FilterOptions;
  onChange: (filters: MatchFilters) => void;
};

export function FilterBar({ filters, options, onChange }: Props) {
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [dateFrom, setDateFrom] = useState(filters.dateFrom ?? "");
  const [dateTo, setDateTo] = useState(filters.dateTo ?? "");
  const [playerQuery, setPlayerQuery] = useState(filters.playerName ?? "");
  const [playerOptions, setPlayerOptions] = useState<PlayerSearchOption[]>([]);
  const [playerSearchOpen, setPlayerSearchOpen] = useState(false);
  const [playerSearchLoading, setPlayerSearchLoading] = useState(false);
  const chips = activeMatchFilterChips(filters, options);
  const patch = (next: Partial<MatchFilters>) => onChange(cleanFilters({ ...filters, ...next }));

  useEffect(() => {
    setDateFrom(filters.dateFrom ?? "");
    setDateTo(filters.dateTo ?? "");
  }, [filters.dateFrom, filters.dateTo]);

  useEffect(() => {
    setPlayerQuery(filters.playerName ?? "");
    setPlayerOptions([]);
  }, [filters.playerName]);

  useEffect(() => {
    const query = playerQuery.trim();
    if (filters.playerName === query || query.length < 2) {
      setPlayerOptions([]);
      setPlayerSearchLoading(false);
      return;
    }
    let cancelled = false;
    setPlayerSearchLoading(true);
    const timeout = window.setTimeout(() => {
      void searchMatchPlayerOptions(query)
        .then((items) => {
          if (!cancelled) setPlayerOptions(items);
        })
        .catch(() => {
          if (!cancelled) setPlayerOptions([]);
        })
        .finally(() => {
          if (!cancelled) setPlayerSearchLoading(false);
        });
    }, 180);
    return () => {
      cancelled = true;
      window.clearTimeout(timeout);
    };
  }, [filters.playerName, playerQuery]);

  function commitDates() {
    patch({ dateFrom: emptyOrValue(dateFrom), dateTo: emptyOrValue(dateTo) });
  }

  function changePlayerQuery(value: string) {
    setPlayerQuery(value);
    setPlayerSearchOpen(true);
    if (filters.playerName && value.trim() !== filters.playerName) patch({ playerName: undefined });
  }

  function selectPlayer(playerName: string) {
    setPlayerQuery(playerName);
    setPlayerSearchOpen(false);
    setPlayerOptions([]);
    patch({ playerName });
  }

  function clearPlayer() {
    setPlayerQuery("");
    setPlayerOptions([]);
    setPlayerSearchOpen(false);
    patch({ playerName: undefined });
  }

  return (
    <section className="filterBar phase6FilterBar" aria-label="对局筛选">
      <MatchFilterToolbar
        filters={filters}
        options={options}
        playerQuery={playerQuery}
        playerOptions={playerOptions}
        playerSearchOpen={playerSearchOpen}
        playerSearchLoading={playerSearchLoading}
        advancedCount={countAdvancedMatchFilters(filters)}
        advancedOpen={advancedOpen}
        hasVisibleFilters={chips.length > 0}
        onPatch={patch}
        onPlayerQueryChange={changePlayerQuery}
        onPlayerFocus={() => setPlayerSearchOpen(true)}
        onPlayerBlur={() => window.setTimeout(() => setPlayerSearchOpen(false), 120)}
        onPlayerKeyDown={(key) => {
          if (key === "Enter" && playerOptions[0]) selectPlayer(playerOptions[0].playerName);
          if (key === "Escape") setPlayerSearchOpen(false);
        }}
        onPlayerSelect={selectPlayer}
        onPlayerClear={clearPlayer}
        onToggleAdvanced={() => setAdvancedOpen((current) => !current)}
        onClear={() => onChange(clearVisibleMatchFilters(filters))}
      />
      {advancedOpen ? (
        <AdvancedMatchFilters
          filters={filters}
          options={options}
          dateFrom={dateFrom}
          dateTo={dateTo}
          onPatch={patch}
          onDateFromChange={setDateFrom}
          onDateToChange={setDateTo}
          onCommitDates={commitDates}
        />
      ) : null}
      <ActiveFilterChips chips={chips} onRemove={(key) => onChange(removeMatchFilter(filters, key))} />
    </section>
  );
}

function cleanFilters(filters: MatchFilters): MatchFilters {
  return Object.fromEntries(Object.entries(filters).filter(([, value]) => value !== undefined && value !== "")) as MatchFilters;
}

function emptyOrValue(value: string) {
  return value.trim() ? value : undefined;
}
