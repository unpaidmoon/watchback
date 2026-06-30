import {
  Activity,
  BarChart3,
  Bell,
  CheckCircle2,
  ChevronRight,
  Clock3,
  Command,
  Database,
  Filter,
  Gauge,
  LayoutDashboard,
  RefreshCw,
  Search,
  ShieldCheck,
  Sparkles,
  Trophy,
  UsersRound
} from "lucide-react";
import { HeroAvatar, MapPreview, MetricCard, StatusBadge } from "../shared/phase6Components";

const heroSamples = [
  { key: "ana", name: "安娜", stat: "治疗 13,840" },
  { key: "genji", name: "源氏", stat: "击杀 31" },
  { key: "winston", name: "温斯顿", stat: "承伤 18,420" }
];

const mapSamples = [
  { key: "blizzard-world", name: "暴雪世界", value: "胜率 64.2%" },
  { key: "busan", name: "釜山", value: "12 场" },
  { key: "kings-row", name: "国王大道", value: "近 7 天高频" }
];

const navItems = ["指挥台", "对局", "复盘", "排行", "队友"];
const metrics = [
  { label: "本赛季对局", value: "186", trend: "已同步至 06-28" },
  { label: "详情覆盖", value: "142", trend: "可复盘 76%" },
  { label: "近期胜率", value: "58.4%", trend: "近 30 场 +6.1%" },
  { label: "高光对局", value: "24", trend: "S/A 评级" }
];

function HeroStack() {
  return (
    <div className="phase6HeroStack" aria-label="英雄样例">
      {heroSamples.map((hero) => (
        <div className="phase6HeroChip" key={hero.name}>
          <HeroAvatar heroKey={hero.key} name={hero.name} size="sm" />
          <span>{hero.name}</span>
        </div>
      ))}
    </div>
  );
}

function MetricStrip() {
  return (
    <section className="phase6MetricStrip" aria-label="赛季指标">
      {metrics.map((metric) => (
        <MetricCard key={metric.label} label={metric.label} value={metric.value} meta={metric.trend} />
      ))}
    </section>
  );
}

function MatchPreviewList() {
  return (
    <div className="phase6MatchList" aria-label="对局样例">
      {mapSamples.map((map, index) => (
        <article className="phase6MatchRow" key={map.name}>
          <MapPreview mapKey={map.key} name={map.name} className="phase6MapThumb" />
          <div>
            <strong>{map.name}</strong>
            <span>{index === 1 ? "失败 · 控制 · 14 分钟前" : "胜利 · 竞技 · 昨日"}</span>
          </div>
          <HeroStack />
          <b>{index === 1 ? "惜败局" : "带飞局"}</b>
        </article>
      ))}
    </div>
  );
}

function TopCapsuleVariant() {
  return (
    <article className="phase6Variant phase6TopVariant">
      <header className="phase6CapsuleBar">
        <div className="phase6BrandMark">
          <Sparkles size={18} />
          <span>本地战绩工作台</span>
        </div>
        <nav aria-label="顶部导航样例">
          {navItems.map((item) => (
            <button className={item === "指挥台" ? "active" : ""} key={item} type="button">
              {item}
            </button>
          ))}
        </nav>
        <button className="phase6IconGhost" type="button" aria-label="通知">
          <Bell size={17} />
        </button>
      </header>
      <div className="phase6VariantIntro">
        <div>
          <span className="phase6Eyebrow">方案一</span>
          <h2>顶部胶囊导航版</h2>
          <p>适合轻量商业产品首页，第一屏更像高级数据仪表盘，导航存在感克制。</p>
        </div>
        <button className="phase6PrimaryAction" type="button">
          同步赛季 <RefreshCw size={16} />
        </button>
      </div>
      <MetricStrip />
      <MatchPreviewList />
    </article>
  );
}

function LeftRailVariant() {
  return (
    <article className="phase6Variant phase6RailVariant">
      <aside className="phase6Rail">
        <div className="phase6RailLogo">
          <Gauge size={18} />
        </div>
        {[LayoutDashboard, ShieldCheck, BarChart3, Trophy, UsersRound].map((Icon, index) => (
          <button className={index === 0 ? "active" : ""} key={index} type="button" aria-label={`导航 ${index + 1}`}>
            <Icon size={18} />
          </button>
        ))}
      </aside>
      <section className="phase6RailCanvas">
        <header className="phase6StatusTop">
          <div>
            <span className="phase6Eyebrow">方案二</span>
            <h2>左侧导航 + 顶部状态栏版</h2>
          </div>
          <div className="phase6StatusPills">
            <StatusBadge tone="good">
              <CheckCircle2 size={14} /> 已登录
            </StatusBadge>
            <StatusBadge tone="good">
              <Database size={14} /> 数据就绪
            </StatusBadge>
            <StatusBadge>
              <Clock3 size={14} /> 06-28 05:40
            </StatusBadge>
          </div>
        </header>
        <div className="phase6RailGrid">
          <section className="phase6GlassPanel">
            <h3>今日重点</h3>
            <p>釜山控制图胜率下滑，建议优先查看同角色对比和队友组合。</p>
            <HeroStack />
          </section>
          <section className="phase6GlassPanel phase6MapPanel">
            <MapPreview mapKey="busan" name="釜山" className="phase6WideMap" />
            <div>
              <span>地图洞察</span>
              <strong>釜山 · 近期承压</strong>
            </div>
          </section>
        </div>
        <MatchPreviewList />
      </section>
    </article>
  );
}

function HybridCommandVariant() {
  return (
    <article className="phase6Variant phase6HybridVariant">
      <header className="phase6CommandHeader">
        <div>
          <span className="phase6Eyebrow">方案三</span>
          <h2>混合命令中心版</h2>
        </div>
        <label className="phase6SearchBox">
          <Search size={16} />
          <span>搜索英雄、地图、对局标签</span>
        </label>
        <button className="phase6PrimaryAction" type="button">
          打开复盘 <ChevronRight size={16} />
        </button>
      </header>
      <div className="phase6CommandGrid">
        <section className="phase6CommandHero">
          <div>
            <Command size={22} />
            <h3>赛季指挥台</h3>
            <p>把登录、同步、比赛、复盘、排行和分析模型聚合成一个可行动首页。</p>
          </div>
          <MetricStrip />
        </section>
        <section className="phase6EvidencePanel">
          <header>
            <span>
              <Filter size={14} /> 复盘证据
            </span>
            <b>高优先级</b>
          </header>
          {heroSamples.map((hero, index) => (
            <article key={hero.name}>
              <HeroAvatar heroKey={hero.key} name={hero.name} size="sm" />
              <div>
                <strong>{hero.name}</strong>
                <span>{hero.stat}</span>
              </div>
              <small>{index === 0 ? "稳定优势" : "建议复看"}</small>
            </article>
          ))}
        </section>
      </div>
      <div className="phase6MapCards">
        {mapSamples.map((map) => (
          <article key={map.name}>
            <MapPreview mapKey={map.key} name={map.name} className="phase6CardMap" />
            <div>
              <strong>{map.name}</strong>
              <span>{map.value}</span>
            </div>
          </article>
        ))}
      </div>
    </article>
  );
}

export function Phase6StyleBoardPage() {
  return (
    <div className="phase6Board">
      <header className="phase6BoardHeader">
        <div>
          <span className="phase6Eyebrow">第六阶段样式板</span>
          <h1>浅色玻璃磨砂商业数据工作台</h1>
          <p>下面是三种可落地的外壳方向，用真实英雄头像和地图预览图验证质感、信息层级和中文密度。</p>
        </div>
        <div className="phase6BoardStatus">
          <StatusBadge>
            <Activity size={15} /> 原型模式
          </StatusBadge>
          <StatusBadge tone="good">
            <CheckCircle2 size={15} /> 可刷新预览
          </StatusBadge>
        </div>
      </header>

      <TopCapsuleVariant />
      <LeftRailVariant />
      <HybridCommandVariant />
    </div>
  );
}
