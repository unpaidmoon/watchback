import type { PlayerQueryCard, PlayerQueryCardImages, PlayerQueryMemoryImage } from "./playerQueryTypes";

const memoryImageUrl = (image: PlayerQueryMemoryImage | null) =>
  image &&
  /^(image\/png|image\/jpeg|image\/webp|image\/gif)$/.test(image.mimeType) &&
  /^[A-Za-z0-9+/]*={0,2}$/.test(image.dataBase64)
    ? `data:${image.mimeType};base64,${image.dataBase64}`
    : null;

export function formatQueryDuration(seconds: number | null): string {
  if (seconds == null) return "-";
  return `${Math.floor(seconds / 3_600)}小时`;
}

export function PlayerProfileCard({
  card,
  images,
  battleTag,
}: {
  card: PlayerQueryCard;
  images?: PlayerQueryCardImages | null;
  battleTag: string;
}) {
  const icon = memoryImageUrl(images?.icon ?? null);
  return (
    <article className="playerQueryProfileCard">
      <div className="playerQueryProfileIdentity">
        {icon ? (
          <img src={icon} alt="玩家头像" />
        ) : (
          <span className="playerQueryImagePlaceholder" aria-label="暂无玩家头像" />
        )}
        <div className="playerQueryIdentityCopy">
          <h2>{battleTag}</h2>
          {card.title ? <p className="playerQueryTitle">{card.title}</p> : null}
        </div>
      </div>
      <dl>
        <div>
          <dt>赞赏等级</dt>
          <dd>{card.level ?? "-"}</dd>
        </div>
        <div>
          <dt>游戏时长</dt>
          <dd>{formatQueryDuration(card.gameTimeSeconds)}</dd>
        </div>
      </dl>
    </article>
  );
}
