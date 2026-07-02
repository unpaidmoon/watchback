import { Search, X } from "lucide-react";
import { useEffect, useState } from "react";
import { searchMatchPlayerOptions } from "../../shared/api";
import { modeLabel } from "../../shared/rankingUtils";
import type { FilterOptions, MatchFilters, PlayerSearchOption } from "../../shared/types";

type Props = {
  filters: MatchFilters;
  options: FilterOptions;
  onChange: (filters: MatchFilters) => void;
};

const resultLabels: Record<string, string> = {
  win: "胜利",
  loss: "失败",
  draw: "平局",
};

const primaryTags = ["带飞局", "尽力局", "躺赢局", "背锅局"];

export function FilterBar({ filters, options, onChange }: Props) {
  const [dateFrom, setDateFrom] = useState(filters.dateFrom ?? "");
  const [dateTo, setDateTo] = useState(filters.dateTo ?? "");
  const [playerQuery, setPlayerQuery] = useState(filters.playerName ?? "");
  const [playerOptions, setPlayerOptions] = useState<PlayerSearchOption[]>([]);
  const [playerSearchOpen, setPlayerSearchOpen] = useState(false);
  const [playerSearchLoading, setPlayerSearchLoading] = useState(false);
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

  function commitDates(nextFrom = dateFrom, nextTo = dateTo) {
    patch({ dateFrom: emptyOrValue(nextFrom), dateTo: emptyOrValue(nextTo) });
  }

  function changePlayerQuery(value: string) {
    setPlayerQuery(value);
    setPlayerSearchOpen(true);
    if (filters.playerName && value.trim() !== filters.playerName) {
      patch({ playerName: undefined });
    }
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
      <div className="phase6FilterRow phase6FilterRowPrimary">
        <label>
          赛季
          <select value={filters.season ?? ""} onChange={(event) => patch({ season: numberOrUndefined(event.target.value) })}>
            <option value="">全部</option>
            {options.seasons.map((season) => (
              <option key={season} value={season}>
                S{season}
              </option>
            ))}
          </select>
        </label>
        <label>
          英雄
          <select value={filters.hero ?? ""} onChange={(event) => patch({ hero: emptyOrValue(event.target.value) })}>
            <option value="">全部</option>
            {options.heroes.map((hero) => (
              <option key={hero.id} value={hero.id}>
                {hero.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          地图
          <select value={filters.map ?? ""} onChange={(event) => patch({ map: emptyOrValue(event.target.value) })}>
            <option value="">全部</option>
            {options.maps.map((map) => (
              <option key={map.id} value={map.id}>
                {map.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          胜负
          <select value={filters.result ?? ""} onChange={(event) => patch({ result: emptyOrValue(event.target.value) })}>
            <option value="">全部</option>
            {options.results.map((result) => (
              <option key={result} value={result}>
                {resultLabels[result] ?? result}
              </option>
            ))}
          </select>
        </label>
        <label>
          模式
          <select value={filters.mode ?? ""} onChange={(event) => patch({ mode: emptyOrValue(event.target.value) })}>
            <option value="">全部</option>
            {options.modes.map((mode) => (
              <option key={mode} value={mode}>
                {modeLabel(mode)}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="phase6FilterRow phase6FilterRowSecondary">
        <label>
          复盘标签
          <select value={filters.primaryTag ?? ""} onChange={(event) => patch({ primaryTag: emptyOrValue(event.target.value) })}>
            <option value="">全部</option>
            {primaryTags.map((tag) => (
              <option key={tag} value={tag}>
                {tag}
              </option>
            ))}
          </select>
        </label>
        <label>
          开始日期
          <input
            type="date"
            value={dateFrom}
            onChange={(event) => setDateFrom(event.target.value)}
            onBlur={() => commitDates()}
            onKeyDown={(event) => {
              if (event.key === "Enter") commitDates();
            }}
          />
        </label>
        <label>
          结束日期
          <input
            type="date"
            value={dateTo}
            onChange={(event) => setDateTo(event.target.value)}
            onBlur={() => commitDates()}
            onKeyDown={(event) => {
              if (event.key === "Enter") commitDates();
            }}
          />
        </label>
        <label className="phase6PlayerSearchLabel">
          玩家展示名
          <span className="phase6PlayerSearchBox">
            <Search size={15} aria-hidden="true" />
            <input
              type="search"
              value={playerQuery}
              placeholder="输入玩家展示名"
              onChange={(event) => changePlayerQuery(event.target.value)}
              onFocus={() => setPlayerSearchOpen(true)}
              onBlur={() => window.setTimeout(() => setPlayerSearchOpen(false), 120)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && playerOptions[0]) selectPlayer(playerOptions[0].playerName);
                if (event.key === "Escape") setPlayerSearchOpen(false);
              }}
            />
            {filters.playerName && (
              <button type="button" className="phase6PlayerSearchClear" onClick={clearPlayer} aria-label="清除玩家展示名筛选">
                <X size={14} />
              </button>
            )}
          </span>
          {playerSearchOpen && playerQuery.trim().length >= 2 && !filters.playerName && (
            <span className="phase6PlayerSearchMenu">
              {playerSearchLoading && <span className="phase6PlayerSearchState">搜索中...</span>}
              {!playerSearchLoading &&
                playerOptions.map((item) => (
                  <button type="button" key={item.playerName} onMouseDown={(event) => event.preventDefault()} onClick={() => selectPlayer(item.playerName)}>
                    <strong>{item.playerName}</strong>
                    <small>
                      {item.matches} 场{item.latestBeginTs ? ` / ${formatLatestBegin(item.latestBeginTs)}` : ""}
                    </small>
                  </button>
                ))}
              {!playerSearchLoading && playerOptions.length === 0 && <span className="phase6PlayerSearchState">未找到玩家展示名</span>}
            </span>
          )}
        </label>
      </div>
    </section>
  );
}

function cleanFilters(filters: MatchFilters): MatchFilters {
  return Object.fromEntries(Object.entries(filters).filter(([, value]) => value !== undefined && value !== "")) as MatchFilters;
}

function emptyOrValue(value: string) {
  return value.trim() ? value : undefined;
}

function numberOrUndefined(value: string) {
  return value ? Number(value) : undefined;
}

function formatLatestBegin(value: number) {
  const seconds = value > 10_000_000_000 ? Math.floor(value / 1000) : value;
  const date = new Date(seconds * 1000);
  if (Number.isNaN(date.getTime())) return "";
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${month}-${day}`;
}
