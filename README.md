# astrbot_plugin_owstats777

## OWStatsLocal Windows 桌面应用

当前产品主线是 `apps/owstats-desktop` 下的 Windows 优先 Tauri 桌面应用。桌面端是单用户、本地 SQLite 数据路径，不做多账号、多玩家，也不把桌面功能回流到 Astrbot。

首次使用流程：

1. 打开桌面应用，进入“设置”。
2. 点击“登录网易大神”，应用会调用 Tauri 后端打开 Chrome 或 Edge。
3. 登录完成后，凭证只保存在本机应用数据目录；界面只显示状态、role_id 和保存时间，不显示 token、cookie、customerToken 或原始凭证内容。
4. 选择赛季并点击“同步赛季”，同步结果进入本地 SQLite。

主要功能：

- “对局”是唯一筛选入口，支持赛季、英雄、地图、胜负、模式、开始日期、结束日期、近 7 天、近 30 天和全部清空；筛选状态写入本机 URL query。
- “对局详情”在原详情下方显示同职责本赛季均值差值，职责使用上游 `roleType`：`tank`、`dps`、`healer`。
- “排行”提供英雄胜率和地图胜率两个 tab，后端聚合场数、胜场、负场、胜率和游玩时长，前端只展示并支持名称搜索。
- “分析”不再提供筛选，默认使用当前已同步赛季全量数据；上排保留好友分析，下排显示四张地图分析卡。
- “复盘”保留个人赛季复盘，其旧地图适应性区域已迁移到排行的地图胜率模块。

开发和诊断脚本仍保留：`scripts/fetch_dashen_credentials.py`、`scripts/fetch_player_data.py`、`scripts/download_ow_assets.py`。这些脚本只作为开发/排障工具，安装版产品路径优先使用 Tauri 后端。

Astrbot 插件，封装 [Overstats](https://github.com/AddOneSecondL/Overstats) 本地 API，为 QQ/Telegram 等平台提供守望先锋数据查询功能。

> Forked from [AddOneSecondL/Overstats](https://github.com/AddOneSecondL/Overstats)

## 功能特性

- 玩家资料、战绩详情、段位历史查询
- 今日/昨日/周总结图片生成
- 快速/竞技强度分析
- AI 锐评（基于 Astrbot LLM）
- AI 开庭（审判视角分析）
- 省榜/英雄榜/选取率排行
- 商店、赛事、补丁查询
- 用户 BattleTag 绑定

## 安装方式

### 方式一：内置版（推荐）

内置版包含完整 Overstats 服务，无需额外部署。

1. 从 `release/` 目录下载 `astrbot_plugin_owstats777_bundled.zip`
2. 在 Astrbot WebUI 上传插件（或解压到插件目录）
3. 重启 Astrbot
4. 在插件配置中填写大神 `dashen_role_id` 和 `dashen_token`
5. 发送 `ow help` 测试

### 方式二：普通版（暂不开放）

普通版需要单独部署 Overstats 服务。

1. 从 `release/` 目录下载 `astrbot_plugin_owstats777.zip`
2. 在 Astrbot WebUI 上传插件
3. 部署 Overstats 服务（参考 `overstats-ubuntu.tar.gz`）
4. 在插件配置中填写 `overstats_url`
5. 发送 `ow help` 测试

## 命令列表

| 命令 | 说明 |
|------|------|
| `ow help` | 显示帮助 |
| `ow 资料 [玩家]` | 玩家资料图 |
| `ow 战绩 [玩家]` | 近期战绩图 |
| `ow 详情 [玩家] [序号]` | 单场详情图 |
| `ow 详情 [玩家] [序号] 锐评` | 详情 + AI 锐评 |
| `ow 开庭 [玩家] [序号]` | AI 开庭（审判视角） |
| `ow 段位 [玩家]` | 段位历史图 |
| `ow 今日/昨日/周 [玩家]` | 总结图 |
| `ow 强度 [玩家]` | 快速强度分析 |
| `ow 竞技强度 [玩家]` | 竞技强度分析 |
| `ow 同玩 [玩家1] [玩家2]` | 同玩查询 |
| `ow 排行 省榜 [省] [职责]` | 省榜排名 |
| `ow 排行 英雄 [省] [英雄]` | 英雄榜单 |
| `ow 排行 选取率 [模式]` | 英雄选取率 |
| `ow 绑定 [BattleTag]` | 绑定 BattleTag |
| `ow 解绑` | 解除绑定 |
| `ow 我的` | 查看绑定信息 |
| `ow 商店` | 当前商店商品 |
| `ow 赛事` | OWCS 赛事信息 |
| `ow 补丁` | 最新补丁说明 |

## 配置说明

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `overstats_url` | `http://127.0.0.1:18080` | Overstats 服务地址 |
| `overstats_port` | `18080` | 内置服务端口 |
| `embedded_overstats` | `true` | 是否启用内置服务 |
| `dashen_role_id` | 空 | 大神 role_id |
| `dashen_token` | 空 | 大神 token |
| `default_timeout` | `120` | 默认请求超时（秒） |
| `ai_timeout` | `180` | AI 锐评/开庭超时（秒） |
| `ai_whitelist` | `[]` | AI 白名单用户列表 |
| `ai_cooldown_seconds` | `300` | AI 冷却时间（秒） |

## 大神 ROLE_ID 获取

1. 打开大神充值中心：https://pay.ds.163.com/activity/ld5?addSkeleton=1&channel=yydc_cps10.cczx&setId=67b466a5b8cf7e3576aa3521
2. 确认已登录并绑定战网账号
3. F12 打开开发者工具 → Network（网络）
4. Ctrl+F5 强制刷新页面
5. 在网络请求中搜索 `&role_id`
6. 复制 `role_id` 后面的数字（如 `114514191`）

## 大神 Token 获取

1. 打开大神主页 https://ds.163.com/
2. F12 打开控制台
3. 运行以下脚本（替换 `YOUR_ROLE_ID`）：

```javascript
(async () => {
  const url = "https://inf.ds.163.com/v1/web/game/report/getReportToken";
  const payload = {
    appKey: "bn",
    roleId: "YOUR_ROLE_ID",
    server: "1",
    source: 1,
    type: "yearly",
  };
  function getCookie(name) {
    return (
      document.cookie
        .split("; ")
        .find((row) => row.startsWith(name + "="))
        ?.split("=")
        .slice(1)
        .join("=") || ""
    );
  }
  const body = JSON.stringify(payload);
  const sigMod = await window.sig.default();
  const signRaw = sigMod.gen_sign(body);
  const signObj = JSON.parse(signRaw);
  const xsrf = getCookie("GL-XSRF-TOKEN");
  const uid = getCookie("GOD_UUID");
  const deviceId =
    localStorage.getItem("ns-client-id") ||
    localStorage.getItem("ds-website-uuid") ||
    "";
  const resp = await fetch(url, {
    method: "POST",
    credentials: "include",
    headers: {
      "Content-Type": "application/json;charset=UTF-8",
      "GL-ClientType": "61",
      "GL-DeviceId": deviceId,
      "GL-Uid": uid,
      "GL-X-XSRF-TOKEN": xsrf,
      "GL-CheckSum": signObj.sign,
      "GL-Nonce": String(signObj.timestamp),
    },
    body,
  });
  const text = await resp.text();
  try {
    const json = JSON.parse(text);
    console.log("role_id =", json?.result?.roleId || payload.roleId);
    console.log("token =", json?.result?.token || "");
  } catch (e) {
    console.log("not json");
  }
})();
```

4. 将输出的 `role_id` 和 `token` 填入插件配置

## 已验证环境

- **Windows 11**：Python 3.10 / 3.12  
  支持 AstrBot Launcher（x86_64）

- **Ubuntu 24.04**：Python 3.13（x86_64，uv 管理 Astrbot 部署）  

- **OpenWRT / iStoreOS 24.10.2**：Docker 部署  
  （Rockchip RK3399，NanoPi R4s，arm64）

- **Astrbot 版本**：v4.25.5

## 排障

常见问题请参考 [TROUBLESHOOTING.md](TROUBLESHOOTING.md)

