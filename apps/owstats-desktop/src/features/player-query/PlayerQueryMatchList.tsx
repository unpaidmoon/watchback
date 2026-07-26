import { MatchListCard } from "../matches/MatchListCard";
import type { PlayerQueryMatchRow } from "./playerQueryTypes";

export function PlayerQueryMatchList({ matches, onOpenMatch, onOpenMore }: { matches: readonly PlayerQueryMatchRow[]; onOpenMatch: (match: PlayerQueryMatchRow) => void; onOpenMore: () => void }) {
  if (matches.length === 0) return <div className="playerQueryLocalEmpty">该赛季该模式暂无对局</div>;
  return <><div className="playerQueryMatchRows">{matches.slice(0, 12).map((match) => <MatchListCard key={match.matchId} variant="playerQuery" compact match={match} selected={false} onSelect={() => onOpenMatch(match)} />)}</div><button className="secondaryButton playerQueryMoreButton" type="button" onClick={onOpenMore}>更多对局</button></>;
}
