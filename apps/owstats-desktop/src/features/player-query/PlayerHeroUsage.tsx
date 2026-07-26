import { HeroAvatar } from "../../shared/phase6Components";
import type { PlayerQueryHeroUsage as HeroUsage } from "./playerQueryTypes";
import { formatQueryDuration } from "./PlayerProfileCard";
import { resolveHeroDisplayName } from "../../shared/assets";

export function PlayerHeroUsage({ heroes }: { heroes: readonly HeroUsage[] }) {
  if (heroes.length === 0) return <div className="playerQueryLocalEmpty">该赛季该模式暂无英雄数据</div>;
  const top = Math.max(...heroes.map((hero) => hero.gameTimeSeconds), 1);
  return <div className="playerQueryHeroRows">{heroes.map((hero) => { const winRate = hero.winRate ?? (hero.games && hero.wins != null ? hero.wins / hero.games : null); const heroName = resolveHeroDisplayName(hero.heroGuid, hero.heroName); return <article className="playerQueryHeroRow" key={hero.heroGuid || hero.heroName}><HeroAvatar heroKey={hero.heroGuid} name={heroName} size="sm" /><div className="playerQueryHeroCopy"><div><strong>{heroName}</strong><span>{winRate == null ? "-" : `${Math.round(winRate <= 1 ? winRate * 100 : winRate)}%`}</span></div><div className="playerQueryHeroProgress" role="progressbar" aria-label={`${heroName}使用时长`} aria-valuemin={0} aria-valuemax={top} aria-valuenow={hero.gameTimeSeconds}><span style={{ transform: `scaleX(${hero.gameTimeSeconds / top})` }} /></div><small>{formatQueryDuration(hero.gameTimeSeconds)}</small></div></article>; })}</div>;
}
