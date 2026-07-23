import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { achievementBadgeUrl } from "../../shared/assets";
import { GlassPanel } from "../../shared/phase6Components";
import { activeBadgeLevel, formatAchievementUnlockDate, romanLevels } from "../../shared/reviewAchievements";
import type { CareerAchievement } from "../../shared/types";

export function AchievementWall({ achievements }: { achievements: CareerAchievement[] }) {
  const [selected, setSelected] = useState<CareerAchievement | null>(null);
  return <GlassPanel className="phase6AchievementPanel" tone="solid"><header className="phase6AchievementHeader"><div><h3>成就墙</h3></div></header><div className="phase6AchievementWall">{achievements.map((achievement) => <AchievementCard key={achievement.key} achievement={achievement} onOpen={() => setSelected(achievement)} />)}</div>{selected ? <AchievementDialog achievement={selected} onClose={() => setSelected(null)} /> : null}</GlassPanel>;
}

function AchievementCard({ achievement, onOpen }: { achievement: CareerAchievement; onOpen: () => void }) {
  const activeLevel = activeBadgeLevel(achievement);
  return <article className={`phase6AchievementCard tier-${achievement.currentLevel}`}><button className="phase6AchievementCardButton" type="button" onClick={onOpen}><AchievementGlow /><img className={`phase6AchievementMedalImage ${achievement.currentLevel === 0 ? "locked" : ""}`} src={achievementBadgeUrl(achievement.key, activeLevel)} alt={achievement.title} /><div className="phase6AchievementCopy"><strong>{achievement.title}</strong><small>{achievement.detail}</small></div></button></article>;
}

function AchievementGlow() {
  return <span className="phase6AchievementGlow" aria-hidden="true"><span className="phase6AchievementGlowParticles" /></span>;
}

function AchievementDialog({ achievement, onClose }: { achievement: CareerAchievement; onClose: () => void }) {
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  useEffect(() => { const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null; closeButtonRef.current?.focus(); const handleKeyDown = (event: KeyboardEvent) => { if (event.key === "Escape") { event.preventDefault(); onClose(); } else if (event.key === "Tab") { event.preventDefault(); closeButtonRef.current?.focus(); } }; document.addEventListener("keydown", handleKeyDown); return () => { document.removeEventListener("keydown", handleKeyDown); previous?.focus(); }; }, [onClose]);
  return <div className="phase6AchievementModalBackdrop" role="presentation" onClick={onClose}><section className="phase6AchievementModal" role="dialog" aria-modal="true" aria-label={`${achievement.title} 详情`} onClick={(event) => event.stopPropagation()}><header className="phase6AchievementModalHeader"><h3>{achievement.title}</h3><button ref={closeButtonRef} className="phase6IconButton" type="button" onClick={onClose} aria-label="关闭成就详情"><X size={18} /></button></header><div className="phase6AchievementBadgeTrack">{achievement.tiers.map((achievementTier, index) => { const tier = (index + 1) as 1 | 2 | 3; const unlockDate = formatAchievementUnlockDate(achievementTier.unlockedAt); const unlocked = achievementTier.level <= achievement.currentLevel; return <div key={achievementTier.level} className={`phase6AchievementBadgeStage tier-${tier} ${unlocked ? "unlocked" : "locked"}`}><AchievementGlow /><img src={achievementBadgeUrl(achievement.key, tier)} alt={`${achievement.title} ${romanLevels[index]}`} /><strong>{romanLevels[index]}</strong><span>{achievementTier.requirement}</span>{unlockDate ? <small className="phase6AchievementUnlockDate">{unlockDate}</small> : null}</div>; })}</div></section></div>;
}
