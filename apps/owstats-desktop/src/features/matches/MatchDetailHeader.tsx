import { resultEnglishLabel } from "../../shared/matchDetailHeaderFormat";
import { MapPreview } from "../../shared/phase6Components";

export type MatchDetailHeaderProps = {
  resultLabel: string;
  result: string | null;
  mapGuid: string | null;
  mapName: string;
  modeLabel: string;
  score: string;
  duration: string;
  startTime: string;
  hasPrevious: boolean;
  hasNext: boolean;
  onPrevious: () => void;
  onNext: () => void;
};

export type MatchDetailNavigationProps = Pick<
  MatchDetailHeaderProps,
  "hasPrevious" | "hasNext" | "onPrevious" | "onNext"
>;

export function MatchDetailHeader({
  resultLabel,
  result,
  mapGuid,
  mapName,
  modeLabel,
  score,
  duration,
  startTime,
  hasPrevious,
  hasNext,
  onPrevious,
  onNext,
}: MatchDetailHeaderProps) {
  const tone = result === "win" || result === "loss" || result === "draw" ? result : "neutral";

  return (
    <header className={`matchDetailHeader result-${tone}`}>
      <MapPreview mapKey={mapGuid ?? mapName} name={mapName} className="matchDetailHeaderMap" decorative />
      <div className="matchDetailHeaderScrim" aria-hidden="true" />
      <div className={`matchDetailHeaderResultBlock result-${tone}`}>
        <strong>{resultLabel}</strong>
        <span>{resultEnglishLabel(result)}</span>
      </div>
      <div className="matchDetailHeaderContent">
        <div className="matchDetailHeaderCopy">
          <h2>{mapName}</h2>
          <span className="matchDetailHeaderMeta">
            <span>{modeLabel}</span>
            <span>时长 {duration}</span>
            <span>开始 {startTime}</span>
          </span>
        </div>
        <strong className="matchDetailHeaderScore" data-numeric>{score}</strong>
        <nav className="matchDetailHeaderNavigation" aria-label="浏览相邻对局">
          <button type="button" className="ghostButton" disabled={!hasPrevious} onClick={onPrevious}>上一场</button>
          <button type="button" className="ghostButton" disabled={!hasNext} onClick={onNext}>下一场</button>
        </nav>
      </div>
    </header>
  );
}
