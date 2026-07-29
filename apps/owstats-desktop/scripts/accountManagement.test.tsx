import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountManager, AccountRemovalDialog } from "../src/features/command-center/AccountManager";

const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
const commandCenterStyles = readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8");
const tauriLibSource = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const tauriCommandsSource = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");
const apiSource = readFileSync(new URL("../src/shared/api.ts", import.meta.url), "utf8");
const typesSource = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");
const modelsSource = readFileSync(new URL("../src-tauri/src/models.rs", import.meta.url), "utf8");
const isolationSource = readFileSync(new URL("../src/features/command-center/accountOperationIsolation.ts", import.meta.url), "utf8");
const seasonFetchSource = readFileSync(new URL("../src-tauri/src/season_fetch.rs", import.meta.url), "utf8");
const profileCardSource = readFileSync(new URL("../src-tauri/src/profile_card.rs", import.meta.url), "utf8");
const qrLoginSource = readFileSync(new URL("../src-tauri/src/qr_login.rs", import.meta.url), "utf8");
const sharedProfileCardSource = readFileSync(new URL("../src/features/profile-card/ProfileSummaryCard.tsx", import.meta.url), "utf8");

assert.doesNotMatch(seasonFetchSource, /\.query_card\(/, "season sync must not fetch profile_card");
assert.match(profileCardSource, /\.query_card\(&target\.customer_token\)/);
assert.doesNotMatch(profileCardSource, /\.query_count_info\(/);
assert.match(profileCardSource, /fetch_game_action_for_account[\s\S]*?\.fight_query_count\(&target\.customer_token, None\)/);
assert.match(tauriCommandsSource, /pub async fn refresh_profile_card/);
assert.match(tauriCommandsSource, /profile_card::refresh_for_account/);
assert.match(qrLoginSource, /profile_card::refresh_for_account/);
assert.match(tauriLibSource, /commands::refresh_profile_card/);
assert.match(tauriLibSource, /commands::get_player_game_action/);
assert.match(apiSource, /invoke<PlayerProfile>\("refresh_profile_card"\)/);
assert.match(apiSource, /invoke<PlayerGameAction>\("get_player_game_action"\)/);
assert.match(sharedProfileCardSource, /aria-label=\{avatarAction\.busy \? "正在刷新账号资料" : "点击头像刷新账号资料"\}/);
assert.match(commandCenterStyles, /\.commandCenterProfileRefresh\s*\{[\s\S]*?position:\s*relative;[\s\S]*?cursor:\s*pointer;/);
assert.match(commandCenterStyles, /\.profileSummaryCard \.phase6ProfileAvatar\s*\{[\s\S]*?width:\s*80px;[\s\S]*?height:\s*80px;/);
assert.match(commandCenterStyles, /\.commandCenterTitleIcon img\s*\{[\s\S]*?width:\s*100%;[\s\S]*?aspect-ratio:\s*5 \/ 1;[\s\S]*?object-fit:\s*cover;/);
assert.match(typesSource, /titleIconUrl:\s*string \| null;/);
assert.match(typesSource.match(/export type PlayerProfile\s*=\s*\{[\s\S]*?\};/)?.[0] ?? "", /gameAction:\s*PlayerGameAction \| null/);
assert.match(modelsSource, /pub title_icon_url:\s*Option<String>/);
assert.match(modelsSource.match(/pub struct PlayerProfile\s*\{[\s\S]*?\}/)?.[0] ?? "", /game_action:\s*Option<PlayerGameAction>/);
assert.match(sharedProfileCardSource, /className="commandCenterTitleIcon"/);
assert.match(sharedProfileCardSource, /className="phase6ProfileAvatar commandCenterProfileRefresh"[\s\S]*?onClick=\{avatarAction\.onClick\}/);
assert.doesNotMatch(sharedProfileCardSource, /commandCenterAvatarActions/);
assert.match(sharedProfileCardSource, /nameHeadingId[\s\S]*?commandCenterProfileTitle/);
assert.match(commandCenterStyles, /\.commandCenterProfileTitle\s*\{[\s\S]*?color:\s*var\(--text-primary\)/);
assert.doesNotMatch(commandCenterStyles.match(/\.commandCenterProfileTitle\s*\{[\s\S]*?\}/)?.[0] ?? "", /margin-left:\s*auto/);
assert.match(commandCenterStyles, /\.commandCenterIdentityLine h2\s*\{[\s\S]*?flex:\s*0 1 auto;/);
assert.match(sharedProfileCardSource, /本周表现：[\s\S]*?gameActionLoading[\s\S]*?profile\.gameAction\?\.comment/);
assert.match(onboardingSource, /getPlayerGameAction\(\)[\s\S]*?setGameActionDialogOpen\(true\)/);
assert.match(onboardingSource, /setGameAction\(profile\?\.gameAction \?\? null\)/);
assert.match(profileCardSource, /save_player_game_action/);
assert.match(sharedProfileCardSource, /称赞他人[\s\S]*?收到称赞[\s\S]*?被举报次数/);
assert.match(sharedProfileCardSource, /role="dialog" aria-modal="true" aria-labelledby="game-action-dialog-title"/);
assert.match(commandCenterStyles, /\.gameActionDialogBackdrop\s*\{[\s\S]*?background:\s*rgb\(0 0 0 \/ 0\.68\)/);
assert.match(commandCenterStyles, /\.gameActionDialog\s*\{[\s\S]*?background:\s*color-mix\(in srgb, var\(--surface-secondary\) 96%, var\(--text-primary\) 4%\)/);

assert.match(onboardingSource, /listAccounts,/);
assert.match(onboardingSource, /<AccountManager/);
assert.match(onboardingSource, /window\.location\.reload\(\)/);
assert.match(onboardingSource, /dashboardCache = null/);
assert.match(onboardingSource, /添加账号后即可同步和查看本地数据/);
assert.match(
  onboardingSource,
  /!accountContextInvalidated && accountStatus\?\.activeAccountId/,
  "fail-close 后不得渲染旧账号的同步和查询内容",
);
assert.ok(
  isolationSource.indexOf("const accountStatus = await options.listAccounts()") <
    isolationSource.indexOf("const scopedDashboard = await options.loadAccountScope()"),
  "账户列表必须先于账户域命令加载",
);
assert.match(
  isolationSource,
  /if \(!accountStatus\.activeAccountId\) \{[\s\S]*?status: null,[\s\S]*?profile: null,[\s\S]*?targetStatus: null/,
  "没有活动账号时不得继续调用账户域命令",
);
assert.match(
  isolationSource,
  /cachedSnapshot\?\.accountStatus\.activeAccountId === accountStatus\.activeAccountId/,
  "仪表盘缓存必须绑定活动账号",
);
assert.match(isolationSource, /runContextMutation[\s\S]*?failClosed\(\);[\s\S]*?reload\(\)[\s\S]*?refreshAfterMutation\(\)/);
assert.match(isolationSource, /result\.removed && result\.reloadRequired[\s\S]*?failClosed\(\);[\s\S]*?reload\(\)[\s\S]*?refreshAfterMutation\(\)/);
assert.doesNotMatch(onboardingSource, /runLogout/);
assert.doesNotMatch(onboardingSource, />退出登录</);
assert.doesNotMatch(tauriLibSource, /commands::logout\b/);
assert.doesNotMatch(tauriLibSource, /commands::start_dashen_login\b/);
assert.doesNotMatch(tauriCommandsSource, /pub (?:async )?fn (?:logout|start_dashen_login)\b/);
assert.doesNotMatch(apiSource, /export function (?:logout|startDashenLogin)\b/);
assert.doesNotMatch(onboardingSource, /runReloginAccount|reloginAccount|onRelogin/);
assert.doesNotMatch(isolationSource, /runReloginAccountOperation|kind: "relogin"/);
assert.doesNotMatch(apiSource, /reloginAccount|"relogin_account"/);
assert.doesNotMatch(tauriCommandsSource, /pub async fn relogin_account\b/);
assert.doesNotMatch(tauriLibSource, /commands::relogin_account\b/);
assert.doesNotMatch(typesSource, /type (?:LogoutResult|CredentialLoginResult)\b/);
const accountPublicContracts = typesSource.match(
  /export type AccountSummary[\s\S]*?export type RemoveAccountResult[\s\S]*?};/,
)?.[0] ?? "";
assert.ok(accountPublicContracts, "账号管理公开类型必须存在");
assert.doesNotMatch(
  accountPublicContracts,
  /roleId|role_id|customerToken|authorization|cookie|\btoken\b/i,
  "账号管理公开类型不得声明私密身份或凭据字段",
);
assert.doesNotMatch(modelsSource, /struct (?:LogoutResult|CredentialLoginResult)\b/);
assert.match(tauriLibSource, /commands::cancel_dashen_login\b/);
assert.match(commandCenterStyles, /\.phase6CommandCenter \.accountManagerPanel\s*\{[\s\S]*?background:\s*var\(--glass-secondary\);[\s\S]*?box-shadow:\s*none;/);
assert.match(commandCenterStyles, /@media \(max-width: 900px\)[\s\S]*?\.phase6CommandCenter \.accountManagerRow[\s\S]*?grid-template-columns:\s*1fr/);

const html = renderToStaticMarkup(
  createElement(AccountManager, {
    accounts: [
      {
        accountId: "account-a",
        battleTag: "Tester#5479",
        isActive: true,
        credentialState: "ready",
        createdAt: "",
        lastUsedAt: "2026-07-16T00:00:00Z",
      },
      {
        accountId: "account-b",
        battleTag: "Second#2222",
        isActive: false,
        credentialState: "stale",
        createdAt: "",
        lastUsedAt: "2026-07-15T00:00:00Z",
      },
    ],
    busyAction: null,
    onAdd: () => undefined,
    onSwitch: () => undefined,
    onRemove: () => undefined,
  }),
);

assert.match(html, /添加或更新网易大神账号/);
assert.match(html, /使用全新浏览器会话登录，不会继承上一次账号状态/);
assert.match(html, /Tester#5479/);
assert.match(html, /Second#2222/);
assert.doesNotMatch(html, /重新登录|登录中/);
assert.equal((html.match(/退出账号/g) ?? []).length, 2);
assert.match(html, /disabled=""[^>]*>当前账号</);
assert.match(html, />切换账号</);
assert.doesNotMatch(html, /roleId|token|customerToken/);
assert.match(html, /<h2[^>]*tabindex="-1"[^>]*>账号管理<\/h2>/);
assert.equal((html.match(/accountManagerAddRow/g) ?? []).length, 1, "添加账号保持为独立一行");

const accountActionGroups = [...html.matchAll(/<div class="accountManagerActions" role="group"[^>]*>([\s\S]*?)<\/div>/g)];
assert.equal(accountActionGroups.length, 2, "每个账号行都提供一个具名操作组");
for (const group of accountActionGroups) {
  assert.equal((group[1]?.match(/<button/g) ?? []).length, 2, "每个账号行只保留切换和退出操作");
  assert.match(group[0], /aria-label="[^"]+ 的账号操作"/);
}

const removalDialog = renderToStaticMarkup(
  createElement(AccountRemovalDialog, {
    account: {
      accountId: "account-a",
      battleTag: "Tester#5479",
      isActive: true,
      credentialState: "ready",
      createdAt: "",
      lastUsedAt: "2026-07-16T00:00:00Z",
    },
    busy: false,
    fallbackFocusRef: { current: null },
    onCancel: () => undefined,
    onConfirm: () => undefined,
  }),
);

assert.match(removalDialog, /role="alertdialog"/);
assert.match(removalDialog, /aria-modal="true"/);
assert.match(removalDialog, /aria-labelledby="account-removal-title"/);
assert.match(removalDialog, /aria-describedby="account-removal-description"/);
assert.match(removalDialog, /Tester#5479/);
assert.match(removalDialog, /永久删除该账号保存的凭证信息和对应本地数据库/);
assert.doesNotMatch(removalDialog, /Second#2222|共享设置|浏览器|账号列表/);
assert.match(removalDialog, />取消</);
assert.match(removalDialog, />永久退出账号</);
assert.equal((removalDialog.match(/<button/g) ?? []).length, 2, "确认框只有取消和永久退出两个清晰动作");
