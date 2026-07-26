import { arenaRankAssetUrl, rankAssetUrl } from "../../shared/assets";
import { formatWinRate } from "../../shared/rankingUtils";

export type RoleRankOverviewEntry = {
  roleType: string;
  roleLabel: string;
  matches: number | null;
  winRate: number | null;
  kda: number | null;
  rankSubTier: number | null;
  rankKey: string | null;
};

export type RoleRankOverviewProps = {
  competitive: readonly RoleRankOverviewEntry[];
  openCompetitive?: readonly RoleRankOverviewEntry[];
  stadium: readonly RoleRankOverviewEntry[];
  loading?: boolean;
  error?: string | null;
  emptyMessage?: string;
  showMissingRoles?: boolean;
};

/** Pure rank presentation shared by the persisted review and in-memory player query. */
export function RoleRankOverview({
  competitive,
  openCompetitive,
  stadium,
  loading = false,
  error = null,
  emptyMessage = "暂无段位数据",
  showMissingRoles = false,
}: RoleRankOverviewProps) {
  if (loading) return <div className="phase6EmptyState" role="status">正在加载段位总览...</div>;
  if (error) return <div className="phase6ErrorState" role="alert">{error}</div>;
  if (!showMissingRoles && competitive.length === 0 && (openCompetitive?.length ?? 0) === 0 && stadium.length === 0) {
    return <div className="phase6EmptyState">{emptyMessage}</div>;
  }

  const competitiveRoles = showMissingRoles ? withMissingRoles(competitive) : competitive;
  const combinedCompetitiveRoles = openCompetitive === undefined
    ? competitiveRoles
    : [...competitiveRoles, ...singleOpenRank(openCompetitive, showMissingRoles)];
  return <>
    <RoleRankGroup title="预设竞技与开放竞技" roles={combinedCompetitiveRoles} gridClassName="reviewCompetitiveRankGrid" />
    <RoleRankGroup title="角斗竞技" roles={showMissingRoles ? withMissingRoles(stadium) : stadium} arena />
  </>;
}

const expectedRoles = [
  { roleType: "tank", roleLabel: "重装" },
  { roleType: "damage", roleLabel: "输出" },
  { roleType: "support", roleLabel: "支援" },
] as const;

function withMissingRoles(roles: readonly RoleRankOverviewEntry[]): RoleRankOverviewEntry[] {
  return expectedRoles.map((expected) => roles.find((role) => role.roleType === expected.roleType) ?? {
    ...expected, matches: null, winRate: null, kda: null, rankSubTier: null, rankKey: null,
  });
}

function singleOpenRank(roles: readonly RoleRankOverviewEntry[], withPlaceholder: boolean): RoleRankOverviewEntry[] {
  const open = roles.find((role) => role.roleType === "open") ?? roles[0];
  if (open) return [{ ...open, roleType: "open", roleLabel: "开放" }];
  return withPlaceholder ? [{ roleType: "open", roleLabel: "开放", matches: null, winRate: null, kda: null, rankSubTier: null, rankKey: null }] : [];
}

function RoleRankGroup({ title, roles, arena = false, gridClassName = "" }: { title: string; roles: readonly RoleRankOverviewEntry[]; arena?: boolean; gridClassName?: string }) {
  return <div className="reviewRoleRankGroup">
    <span className="phase6Eyebrow">{title}</span>
    <div className={`phase6RoleRankGrid${arena ? " reviewStadiumRankGrid" : ""}${gridClassName ? ` ${gridClassName}` : ""}`}>
      {roles.map((role) => <RoleRankCard key={`${arena ? "stadium-" : ""}${role.roleType}`} role={role} arena={arena} />)}
    </div>
  </div>;
}

function RoleRankCard({ role, arena = false }: { role: RoleRankOverviewEntry; arena?: boolean }) {
  const tier = role.rankSubTier !== null && role.rankSubTier > 0 ? String(role.rankSubTier) : "";
  const icon = arena ? arenaRankAssetUrl(role.rankKey) ?? rankAssetUrl(role.rankKey) : rankAssetUrl(role.rankKey);
  return <article className="phase6RoleRankCard">
    <div className="phase6RoleRankName">{role.roleLabel}</div>
    <div className="phase6RoleRankIconWrap">
      <img className="phase6RoleRankIcon" src={icon} alt={`${role.roleLabel} ${role.rankKey ?? "未定级"}`} />
      {tier ? <span className="phase6RoleRankTier">{tier}</span> : null}
    </div>
    <div className="phase6RoleRankStats">
      <Metric label="胜率" value={role.winRate === null ? "-" : formatWinRate(role.winRate, 0)} />
      <Metric label="场次" value={role.matches ?? "-"} />
      <Metric label="KDA" value={role.kda === null || role.matches === 0 ? "-" : role.kda.toFixed(2)} />
    </div>
  </article>;
}

function Metric({ label, value }: { label: string; value: string | number }) {
  return <div className="metric"><span>{label}</span><strong>{value}</strong></div>;
}
