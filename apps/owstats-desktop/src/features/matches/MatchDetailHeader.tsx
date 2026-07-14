export type MatchDetailHeaderProps = {
  resultLabel: string;
  result: string | null;
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
      <div className="matchDetailHeaderIdentity">
        <span className="matchDetailHeaderResult">
          <span className="matchDetailHeaderResultMarker" aria-hidden="true" />
          {resultLabel}
        </span>
        <div className="matchDetailHeaderCopy">
          <h2>{mapName} · {modeLabel}</h2>
          <span className="matchDetailHeaderMeta">
            <span>时长 {duration}</span>
            <span>开始 {startTime}</span>
          </span>
        </div>
      </div>
      <strong className="matchDetailHeaderScore" data-numeric>{score}</strong>
      <nav className="matchDetailHeaderNavigation" aria-label="浏览相邻对局">
        <button type="button" className="ghostButton" disabled={!hasPrevious} onClick={onPrevious}>上一场</button>
        <button type="button" className="ghostButton" disabled={!hasNext} onClick={onNext}>下一场</button>
      </nav>
    </header>
  );
}
