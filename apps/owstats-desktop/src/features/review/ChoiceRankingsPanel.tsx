import { resolveArenaModTraitInfo, resolvePerkInfo } from "../../shared/assets";
import { formatWinRate } from "../../shared/rankingUtils";
import type { ChoiceRankingEntry, ChoiceRankings } from "../../shared/types";

type ChoiceKind = "perk" | "stadium";

export function ChoiceRankingsPanel({
  rankings,
  loading,
  error,
}: {
  rankings: ChoiceRankings | null;
  loading: boolean;
  error: string | null;
}) {
  return (
    <section className="phase6ChoiceRankingsPanel" aria-labelledby="choice-rankings-title">
      <header>
        <div>
          <span className="phase6Eyebrow">选择分析</span>
          <h2 id="choice-rankings-title">高胜率选择</h2>
        </div>
        <small>至少选取 {rankings?.minSample ?? 5} 场 · 按 95% 置信统计排序</small>
      </header>
      {loading ? <div className="phase6ChoiceRankingsState">正在加载选择统计...</div> : null}
      {!loading && error ? <div className="phase6ChoiceRankingsState">选择统计加载失败：{error}</div> : null}
      {!loading && !error && !rankings ? <div className="phase6ChoiceRankingsState">暂无可统计数据</div> : null}
      {!loading && !error && rankings ? (
        <div className="phase6ChoiceRankingsGrid">
          <RankingColumn title="普通对局 · 威能" entries={rankings.normalPerks} kind="perk" minSample={rankings.minSample} />
          <RankingColumn title="角斗 · 异能" entries={rankings.stadiumTraits} kind="stadium" minSample={rankings.minSample} />
          <RankingColumn title="角斗 · 装备" entries={rankings.stadiumMods} kind="stadium" minSample={rankings.minSample} />
        </div>
      ) : null}
    </section>
  );
}

function RankingColumn({
  title,
  entries,
  kind,
  minSample,
}: {
  title: string;
  entries: ChoiceRankingEntry[];
  kind: ChoiceKind;
  minSample: number;
}) {
  return (
    <section className="phase6ChoiceRankingColumn">
      <h3>{title}</h3>
      {entries.length === 0 ? (
        <p className="phase6ChoiceRankingsEmpty">样本不足，至少需要 {minSample} 场选取记录</p>
      ) : (
        <ol>
          {entries.map((entry, index) => (
            <RankingRow key={entry.key} entry={entry} rank={index + 1} kind={kind} />
          ))}
        </ol>
      )}
    </section>
  );
}

function RankingRow({ entry, rank, kind }: { entry: ChoiceRankingEntry; rank: number; kind: ChoiceKind }) {
  const info = kind === "perk" ? resolvePerkInfo(entry.key) : resolveArenaModTraitInfo(entry.key);
  const name = info?.name?.trim() || (kind === "perk" ? "未知威能" : entry.key);
  return (
    <li className="phase6ChoiceRankingRow">
      <span className="phase6ChoiceRankingRank">{rank}</span>
      <span className="phase6ChoiceRankingIcon" aria-hidden="true">
        {info?.iconUrl ? <img src={info.iconUrl} alt="" /> : null}
      </span>
      <span className="phase6ChoiceRankingName" title={name}>{name}</span>
      <span className="phase6ChoiceRankingNumbers">
        <strong>{formatWinRate(entry.winRate, 0)}</strong>
        <small>{entry.matches} 场</small>
      </span>
    </li>
  );
}
