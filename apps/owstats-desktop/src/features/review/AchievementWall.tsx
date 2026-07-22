import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { achievementBadgeUrl } from "../../shared/assets";
import { GlassPanel } from "../../shared/phase6Components";
import { activeBadgeLevel, buildReviewAchievements, formatThreshold, romanLevels, type ReviewAchievement } from "../../shared/reviewAchievements";
import type { PersonalSeasonReview } from "../../shared/types";

export function AchievementWall({ review }: { review: PersonalSeasonReview }) {
  const [selected, setSelected] = useState<ReviewAchievement | null>(null);
  return <GlassPanel className="phase6AchievementPanel" tone="solid"><header className="phase6AchievementHeader"><div><h3>成就墙</h3></div></header><div className="phase6AchievementWall">{buildReviewAchievements(review).map((achievement) => <AchievementCard key={achievement.key} achievement={achievement} onOpen={() => setSelected(achievement)} />)}</div>{selected ? <AchievementDialog achievement={selected} onClose={() => setSelected(null)} /> : null}</GlassPanel>;
}

function AchievementCard({ achievement, onOpen }: { achievement: ReviewAchievement; onOpen: () => void }) {
  const activeLevel = activeBadgeLevel(achievement);
  return <article className={`phase6AchievementCard tier-${achievement.currentLevel}`}><button className="phase6AchievementCardButton" type="button" onClick={onOpen}><AchievementGlow /><img className={`phase6AchievementMedalImage ${achievement.currentLevel === 0 ? "locked" : ""}`} src={achievementBadgeUrl(achievement.key, activeLevel)} alt={achievement.title} /><div className="phase6AchievementCopy"><strong>{achievement.title}</strong><small>{achievement.detail}</small></div></button></article>;
}

function AchievementGlow() {
  return <span className="phase6AchievementGlow" aria-hidden="true"><span className="phase6AchievementGlowParticles" /></span>;
}

function AchievementDialog({ achievement, onClose }: { achievement: ReviewAchievement; onClose: () => void }) {
  const closeButtonRef = useRef<HTMLButtonElement>(null); const next = achievement.thresholds[Math.min(achievement.currentLevel, 2)];
  useEffect(() => { const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null; closeButtonRef.current?.focus(); const handleKeyDown = (event: KeyboardEvent) => { if (event.key === "Escape") { event.preventDefault(); onClose(); } else if (event.key === "Tab") { event.preventDefault(); closeButtonRef.current?.focus(); } }; document.addEventListener("keydown", handleKeyDown); return () => { document.removeEventListener("keydown", handleKeyDown); previous?.focus(); }; }, [onClose]);
  return <div className="phase6AchievementModalBackdrop" role="presentation" onClick={onClose}><section className="phase6AchievementModal" role="dialog" aria-modal="true" aria-label={`${achievement.title} 详情`} onClick={(event) => event.stopPropagation()}><header className="phase6AchievementModalHeader"><div><span className="phase6Eyebrow">成就详情</span><h3>{achievement.title}</h3><p>{achievement.detail}</p></div><button ref={closeButtonRef} className="phase6IconButton" type="button" onClick={onClose} aria-label="关闭成就详情"><X size={18} /></button></header><div className="phase6AchievementBadgeTrack">{romanLevels.map((level, index) => { const tier = (index + 1) as 1 | 2 | 3; return <div key={level} className={`phase6AchievementBadgeStage tier-${tier} ${achievement.currentLevel >= tier ? "unlocked" : "locked"} ${achievement.currentLevel === tier || (achievement.currentLevel === 0 && tier === 1) ? "active" : ""}`}><AchievementGlow /><img src={achievementBadgeUrl(achievement.key, tier)} alt={`${achievement.title} ${level}`} /><strong>{level}</strong><span>{formatThreshold(achievement.thresholds[index], achievement.unit)}</span></div>; })}</div><div className="phase6AchievementModalStats"><Metric label="当前" value={achievement.metric} /><Metric label="当前等级" value={achievement.currentLevel ? romanLevels[achievement.currentLevel - 1] : "未激活"} /><Metric label={achievement.currentLevel >= 3 ? "最高等级" : "下一等级"} value={achievement.currentLevel >= 3 ? "III" : formatThreshold(next, achievement.unit)} /></div><div className="phase6AchievementModalProgress"><span style={{ width: `${achievement.progress}%` }} /></div></section></div>;
}
function Metric({ label, value }: { label: string; value: string | number }) { return <div className="metric"><span>{label}</span><strong>{value}</strong></div>; }
