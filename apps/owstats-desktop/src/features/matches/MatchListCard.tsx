import { Star } from "lucide-react";
import {
  formatCompactStat,
  formatMatchPreviewKey,
  formatPlayerQueryDate,
  formatPlayerQueryScoreLine,
  formatScoreLine,
} from "../../shared/matchListFormat";
import { MatchPerformanceBadges } from "../../shared/PerformanceBadgesView";
import { HeroAvatar, MapPreview } from "../../shared/phase6Components";
import { resolveHeroDisplayName, resolveMapDisplayName } from "../../shared/assets";
import type { MatchSummary } from "../../shared/types";

export type PlayerQueryMatchRowView = {
  matchId: string;
  matchKind: "normal" | "arena";
  /** Canonical values are win/loss/draw; string keeps rendering defensive to older DTOs. */
  result: string;
  score: string;
  mapKey?: string | null;
  mapName: string | null;
  heroKey: string | null;
  heroName: string | null;
  kills: number | null;
  assists: number | null;
  deaths: number | null;
  playedAt: string | null;
};

type LocalMatchListCardProps = {
  variant?: "local";
  match: MatchSummary;
  selected: boolean;
  onSelect: (matchId: string) => void;
};

type PlayerQueryMatchListCardProps = {
  variant: "playerQuery";
  match: PlayerQueryMatchRowView;
  selected: boolean;
  onSelect: (matchId: string) => void;
  compact?: boolean;
};

export type MatchListCardProps = LocalMatchListCardProps | PlayerQueryMatchListCardProps;

export function MatchListCard(props: MatchListCardProps) {
  if (props.variant === "playerQuery") {
    return <PlayerQueryMatchListCard {...props} />;
  }

  const { match, selected, onSelect } = props;
  const result = resultPresentation(match.result);
  return (
    <button
      type="button"
      className={`matchListCard matchRow phase6MatchRowReal ${match.result} ${selected ? "phase6MatchRowSelected" : ""}`}
      data-match-id={match.matchId}
      aria-current={selected ? "true" : undefined}
      onClick={() => onSelect(match.matchId)}
    >
      <span className="matchListCardHeader">
        <MapPreview
          mapKey={formatMatchPreviewKey(match)}
          name={match.mapName}
          className="matchListCardHeaderMap"
          decorative
        />
        <span className={`matchListCardResultBlock result-${result.tone}`}>
          <span>{result.label}</span>
          <strong data-numeric>{formatScoreLine(match)}</strong>
        </span>
        <span className="matchListCardBannerCopy" title={`${match.mapName} · ${match.modeLabel}`}>
          <span>{match.mapName}</span>
          <span>{match.modeLabel}</span>
        </span>
        {match.isFavorite ? (
          <span className="matchListFavoriteMark" title="已收藏" aria-label="已收藏">
            <Star size={15} fill="currentColor" aria-hidden="true" />
          </span>
        ) : null}
      </span>
      <span className="matchListCardData">
        <span className="matchListCardAvatar">
          <HeroAvatar heroKey={match.heroGuid || match.heroName} name={match.heroName} />
        </span>
        <MetricBlock label="K/A/D" value={`${match.kills}/${match.assists}/${match.deaths}`} />
        <MetricBlock
          className="matchListCardCombinedStats"
          label="伤害/治疗/抵挡"
          value={[match.damage, match.healing, match.mitigation].map(formatCompactStat).join("/")}
        />
        <span className="matchListCardPerformance">
          <span>对局表现</span>
          <MatchPerformanceBadges evaluation={match.evaluation} teamAwards={match.teamAwards} compact showGrade={false} />
        </span>
      </span>
    </button>
  );
}

function PlayerQueryMatchListCard({ match, selected, onSelect, compact = false }: PlayerQueryMatchListCardProps) {
  const result = resultPresentation(match.result);
  const isArena = match.matchKind === "arena";
  const mapName = resolveMapDisplayName(match.mapKey, match.mapName);
  const heroName = resolveHeroDisplayName(match.heroKey, match.heroName);
  const statistic = (value: number | null) => (value == null ? "-" : String(value));

  if (compact) {
    return (
      <button type="button" className={`matchListCard phase6MatchRowReal playerQueryCompactMatchRow ${result.tone} ${selected ? "phase6MatchRowSelected" : ""}`} data-match-id={match.matchId} aria-current={selected ? "true" : undefined} onClick={() => onSelect(match.matchId)}>
        <span className={`playerQueryCompactResult result-${result.tone}`}>{result.label}</span>
        <strong className="playerQueryCompactScore" data-numeric>{formatPlayerQueryScoreLine(match.score)}</strong>
        {!isArena ? <span className="playerQueryCompactMap" title={mapName}>{mapName}</span> : <span aria-hidden="true" />}
        <HeroAvatar heroKey={match.heroKey || match.heroName} name={heroName} />
        <strong className="playerQueryCompactHero">{heroName}</strong>
        <span className="playerQueryCompactKad" data-numeric>{`${statistic(match.kills)}/${statistic(match.assists)}/${statistic(match.deaths)}`}</span>
        <span className="playerQueryCompactDate" data-numeric>{formatPlayerQueryDate(match.playedAt)}</span>
      </button>
    );
  }

  return (
    <button
      type="button"
      className={`matchListCard matchListCard-playerQuery matchRow phase6MatchRowReal ${result.tone} ${
        selected ? "phase6MatchRowSelected" : ""
      }`}
      data-match-id={match.matchId}
      aria-current={selected ? "true" : undefined}
      onClick={() => onSelect(match.matchId)}
    >
      <span className="matchListCardHeader">
        {isArena ? (
          <img className="phase6MapPreviewShared matchListCardHeaderMap" src="/assets/maps/stadium.png" alt="" aria-hidden="true" />
        ) : (
          <MapPreview mapKey={match.mapKey || match.mapName} name={mapName} className="matchListCardHeaderMap" decorative />
        )}
        <span className={`matchListCardResultBlock result-${result.tone}`}>
          <span>{result.label}</span>
          <strong data-numeric>{formatPlayerQueryScoreLine(match.score)}</strong>
        </span>
        <span className="matchListCardBannerCopy" title={isArena ? "角斗" : mapName}>
          {isArena ? <span className="matchListCardArenaLabel">角斗</span> : <span>{mapName}</span>}
        </span>
      </span>
      <span className="matchListCardData matchListCardQueryData">
        <span className="matchListCardAvatar">
          <HeroAvatar heroKey={match.heroKey || match.heroName} name={heroName} />
        </span>
        <span className="matchListCardQueryHero">
          <span>英雄</span>
          <strong>{heroName}</strong>
        </span>
        <MetricBlock
          label="K/A/D"
          value={`${statistic(match.kills)}/${statistic(match.assists)}/${statistic(match.deaths)}`}
        />
        <MetricBlock label="日期" value={formatPlayerQueryDate(match.playedAt)} />
      </span>
    </button>
  );
}

function resultPresentation(result: string) {
  const normalized = result.trim().toLowerCase();
  if (["win", "victory", "胜利", "1"].includes(normalized)) return { tone: "win", label: "胜利" };
  if (["loss", "lose", "defeat", "失败", "-1", "2"].includes(normalized)) return { tone: "loss", label: "失败" };
  if (["draw", "tie", "平局", "3"].includes(normalized)) return { tone: "draw", label: "平局" };
  return { tone: "neutral", label: "未知结果" };
}

function MetricBlock({ label, value, className = "" }: { label: string; value: string; className?: string }) {
  return (
    <span className={`phase6MatchMetricBlock ${className}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </span>
  );
}
