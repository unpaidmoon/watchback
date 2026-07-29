import { useCallback, useState } from "react";
import { ProfileGameActionDialog, ProfileSummaryCard } from "../profile-card/ProfileSummaryCard";
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
  const titleIcon = memoryImageUrl(images?.titleIcon ?? null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const closeDialog = useCallback(() => setDialogOpen(false), []);
  return (
    <>
      <ProfileSummaryCard
        showTitleIcon={false}
        profile={{
          name: battleTag,
          title: card.title,
          titleIconUrl: titleIcon,
          avatarUrl: icon,
          endorsementLevel: card.level,
          playTimeHours: card.gameTimeSeconds == null ? null : card.gameTimeSeconds / 3_600,
          gameAction: card.gameAction,
        }}
        onGameActionClick={card.gameAction ? () => setDialogOpen(true) : undefined}
      />
      {dialogOpen && card.gameAction ? <ProfileGameActionDialog action={card.gameAction} onClose={closeDialog} /> : null}
    </>
  );
}
