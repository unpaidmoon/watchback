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
};

export function MatchDetailHeader({
  resultLabel,
  result,
  mapGuid,
  mapName,
  modeLabel,
  score,
  duration,
  startTime,
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
        <div className="matchDetailHeaderInfo">
          <h2>{mapName}</h2>
          <strong className="matchDetailHeaderScore" data-numeric>{score}</strong>
          <span className="matchDetailHeaderMeta">
            <span>{modeLabel}</span>
            <span>时长 {duration}</span>
            <span>开始 {startTime}</span>
          </span>
        </div>
      </div>
    </header>
  );
}
