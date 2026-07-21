import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import {
  AI_ROAST_LOADING_INTERVAL_MS,
  AI_ROAST_LOADING_PHRASES,
  AI_ROAST_LIVE_MESSAGE,
  AiMatchRoast,
  classifyAiRoastError,
  clearBusyMatch,
  registerLoadingInterval,
  unregisterLoadingInterval,
} from '../src/features/matches/AiMatchRoast';
import {
  DeepSeekSettings,
  deepSeekStatusLabel,
  nextDeepSeekKeyValue,
  safeDeepSeekSettingsError,
} from '../src/features/command-center/DeepSeekSettings';

const apiSource = readFileSync(new URL('../src/shared/api.ts', import.meta.url), 'utf8');
const typesSource = readFileSync(new URL('../src/shared/types.ts', import.meta.url), 'utf8');
const componentSource = readFileSync(
  new URL('../src/features/matches/AiMatchRoast.tsx', import.meta.url),
  'utf8',
);
const settingsSource = readFileSync(
  new URL('../src/features/command-center/DeepSeekSettings.tsx', import.meta.url),
  'utf8',
);
const onboardingSource = readFileSync(new URL('../src/pages/OnboardingPage.tsx', import.meta.url), 'utf8');
const commandCenterCssSource = readFileSync(
  new URL('../src/styles/features/command-center.css', import.meta.url),
  'utf8',
);

assert.match(typesSource, /export type AiMatchRoastView\s*=\s*\{\s*award:\s*string;\s*commentary:\s*string;\s*\}/s);
assert.match(typesSource, /export type DeepSeekKeyStatus\s*=\s*\{\s*state:\s*\x27ready\x27\s*\|\s*\x27missing\x27;\s*message:\s*string;\s*\}/s);
assert.match(typesSource, /export type AiRoastGenerationResult\s*=\s*\{\s*aiRoast:\s*AiMatchRoastView;\s*\}/s);
assert.match(typesSource, /export type MatchDetailView\s*=\s*\{[\s\S]*?aiRoast:\s*AiMatchRoastView\s*\|\s*null;/);

assert.match(apiSource, /invoke<DeepSeekKeyStatus>\(\x22get_deepseek_key_status\x22\)/);
assert.match(apiSource, /invoke<DeepSeekKeyStatus>\(\x22save_deepseek_key\x22,\s*\{\s*apiKey\s*\}\)/s);
assert.match(apiSource, /invoke<AiRoastGenerationResult>\(\x22generate_match_ai_roast\x22,\s*\{\s*matchId\s*\}\)/s);

const settingsMarkup = renderToStaticMarkup(createElement(DeepSeekSettings));
assert.match(settingsMarkup, new RegExp('AI \\u9510\\u8bc4'));
assert.match(settingsMarkup, new RegExp('\\u672a\\u914d\\u7f6e'));
assert.match(settingsMarkup, /<label[^>]*for=\x22deepseek-api-key\x22[^>]*>DeepSeek API Key<\/label>/);
assert.match(settingsMarkup, /<input[^>]*id=\x22deepseek-api-key\x22[^>]*type=\x22password\x22/);
assert.match(settingsMarkup, /autoComplete=\x22new-password\x22/);
assert.match(settingsMarkup, new RegExp('<button[^>]*>\\u4fdd\\u5b58\\u5e76\\u9a8c\\u8bc1<\\/button>'));
assert.match(settingsMarkup, new RegExp('\\u4ec5\\u53d1\\u9001\\u533f\\u540d\\u5bf9\\u5c40\\u4e8b\\u5b9e\\u3001\\u8bc4\\u4ef7\\u4e0e\\u56de\\u5408\\u8d8b\\u52bf'));
assert.match(settingsMarkup, new RegExp('\\u4e0d\\u4f1a\\u53d1\\u9001\\u73a9\\u5bb6\\u540d\\u79f0\\u3001\\u73a9\\u5bb6 ID\\u3001\\u7b14\\u8bb0\\u3001\\u539f\\u59cb JSON \\u6216\\u5b8c\\u6574\\u9635\\u5bb9'));
assert.doesNotMatch(settingsMarkup, new RegExp('\\u5220\\u9664|\\u6a21\\u578b|\\u98ce\\u683c|Base URL|\\u6d4b\\u8bd5 Key|\\u5c3e\\u53f7|\\u7247\\u6bb5'));
assert.equal(deepSeekStatusLabel({ state: 'ready', message: 'sk-test-fragment' }), '\u5df2\u914d\u7f6e\u5e76\u9a8c\u8bc1');
assert.equal(deepSeekStatusLabel({ state: 'missing', message: 'anything' }), '\u672a\u914d\u7f6e');
assert.equal(nextDeepSeekKeyValue('synthetic-secret', true), '');
assert.equal(nextDeepSeekKeyValue('synthetic-secret', false), 'synthetic-secret');
assert.equal(
  safeDeepSeekSettingsError('C:\\private\\credential-store failed for synthetic-secret'),
  '保存失败，请检查 Key 后重试',
);
assert.equal(safeDeepSeekSettingsError('DeepSeek API Key 无效'), 'DeepSeek API Key 无效，请检查后重试');
assert.match(settingsSource, /useEffect\([\s\S]*?getDeepSeekKeyStatus\(\)/);
assert.match(settingsSource, /hasDesktopBackend\(\)/);
assert.match(settingsSource, /saveDeepSeekKey\(trimmedApiKey\)/);
assert.match(settingsSource, /if\s*\(submitting\)\s*\{\s*return;/);
assert.match(settingsSource, /const trimmedApiKey = apiKey\.trim\(\)/);
assert.match(settingsSource, /setApiKey\(nextDeepSeekKeyValue\(apiKey, true\)\)/);
assert.doesNotMatch(settingsSource, /status\.message|console\.|apiKey[^\n]*(error|feedback|status)/i);
assert.match(settingsSource, /aria-live=\x22polite\x22/);
assert.match(onboardingSource, /import \{ DeepSeekSettings \} from \x22\.\.\/features\/command-center\/DeepSeekSettings\x22/);
assert.match(
  onboardingSource,
  /<section className=\x22commandCenterTools\x22[\s\S]*?<DeepSeekSettings \/>[\s\S]*?<\/section>[\s\S]*?accountStatus\.activeAccountId/s,
  'DeepSeek settings must stay in the tools area outside account-only content',
);
assert.match(commandCenterCssSource, /\.deepSeekSettings[\s\S]*?border-radius:\s*12px/);
assert.match(commandCenterCssSource, /\.deepSeekSettings[^}]*box-shadow:\s*none/);
assert.match(commandCenterCssSource, /\.deepSeekSettings(?:Input|Submit)[\s\S]*?min-height:\s*44px/);

const initialMarkup = renderToStaticMarkup(
  createElement(AiMatchRoast, { matchId: 'match-a', initialRoast: null, detailReady: true }),
);
assert.match(initialMarkup, /class=\x22aiMatchRoast/);
assert.match(initialMarkup, /<button[^>]*>生成 AI 锐评<\/button>/);
assert.doesNotMatch(initialMarkup, /正在生成 AI 锐评/);

const incompleteMarkup = renderToStaticMarkup(
  createElement(AiMatchRoast, { matchId: 'match-b', initialRoast: null, detailReady: false }),
);
assert.match(incompleteMarkup, /<button[^>]*disabled=\x22\x22[^>]*>对局详情完整后可生成锐评<\/button>/);

const savedMarkup = renderToStaticMarkup(
  createElement(AiMatchRoast, {
    matchId: 'match-c',
    initialRoast: { award: '逆风定海针', commentary: '匿名事实一。匿名事实二。' },
    detailReady: true,
  }),
);
assert.match(savedMarkup, /<h3[^>]*>逆风定海针<\/h3>/);
assert.match(savedMarkup, /<p[^>]*>匿名事实一。匿名事实二。<\/p>/);
assert.doesNotMatch(savedMarkup, /本场封号|AI 生成|换一个|重新生成|<button/);

assert.deepEqual(AI_ROAST_LOADING_PHRASES, [
  '正在翻阅本场数据…',
  '正在寻找关键证据…',
  '正在确认谁在带飞…',
  '正在核对锅位归属…',
  '正在申请本场封号…',
  '正在组织锐评措辞…',
]);
assert.equal(AI_ROAST_LOADING_INTERVAL_MS, 2_000);
assert.equal(AI_ROAST_LIVE_MESSAGE, '正在生成 AI 锐评');

assert.deepEqual(classifyAiRoastError('DeepSeek API Key 无效。'), {
  message: 'DeepSeek API Key 无效，请在指挥台重新配置',
  recoverable: false,
});
assert.deepEqual(classifyAiRoastError('DeepSeek 余额不足。'), {
  message: 'DeepSeek 余额不足，请前往 DeepSeek 检查余额',
  recoverable: false,
});
assert.deepEqual(classifyAiRoastError('尚未配置 DeepSeek API Key'), {
  message: '请先在指挥台配置 DeepSeek API Key',
  recoverable: false,
});
assert.equal(classifyAiRoastError('请求过于频繁，请稍后再试。').recoverable, true);
assert.equal(classifyAiRoastError('DeepSeek 服务暂时繁忙，请稍后再试。').recoverable, true);
assert.equal(classifyAiRoastError('无法连接 DeepSeek 服务，请稍后再试。').recoverable, true);
assert.deepEqual(classifyAiRoastError('生成超时，请稍后再试。'), {
  message: '锐评生成超时，请重试',
  recoverable: true,
});
assert.deepEqual(classifyAiRoastError('这场对局暂时没有足够的安全素材生成 AI 锐评。'), {
  message: '锐评输出格式异常，请重试',
  recoverable: true,
});
assert.equal(classifyAiRoastError('对局标识无效。').recoverable, false);
assert.deepEqual(classifyAiRoastError('C:\\private\\db.sqlite synthetic-secret'), {
  message: '锐评生成失败，请稍后重试',
  recoverable: false,
});

assert.equal(
  classifyAiRoastError('\u5df2\u6709\u4e00\u573a AI \u9510\u8bc4\u6b63\u5728\u751f\u6210\uff0c\u8bf7\u7a0d\u540e\u518d\u8bd5').recoverable,
  true,
);

assert.match(componentSource, /getDeepSeekKeyStatus\(\)/, 'key status should be requested lazily by the action');
assert.doesNotMatch(
  componentSource,
  /useEffect\([\s\S]{0,240}getDeepSeekKeyStatus\(\)/,
  'key status must not be fetched eagerly on mount',
);
assert.match(componentSource, /requestTokenRef/);
assert.match(componentSource, /requestMatchId/);
assert.match(componentSource, /requestTokenRef\.current\s*!==\s*requestToken/);
assert.match(componentSource, /requestMatchId\s*!==\s*matchIdRef\.current/);
assert.equal(clearBusyMatch('match-a', 'match-a'), null);
assert.equal(clearBusyMatch('match-b', 'match-a'), 'match-b');

const originalLoadingIntervals = new Set([1]);
const registeredLoadingIntervals = registerLoadingInterval(originalLoadingIntervals, 2);
assert.deepEqual([...originalLoadingIntervals], [1]);
assert.deepEqual([...registeredLoadingIntervals], [1, 2]);
const remainingLoadingIntervals = unregisterLoadingInterval(registeredLoadingIntervals, 1);
assert.deepEqual([...registeredLoadingIntervals], [1, 2]);
assert.deepEqual([...remainingLoadingIntervals], [2]);

assert.match(componentSource, /loadingIntervalsRef\s*=\s*useRef\(\s*new Set</);
assert.match(
  componentSource,
  /loadingIntervalsRef\.current\s*=\s*registerLoadingInterval\(\s*loadingIntervalsRef\.current,\s*loadingInterval,?\s*\)/,
  'each request must register its loading interval',
);
assert.match(
  componentSource,
  /finally\s*\{[\s\S]*?clearInterval\(loadingInterval\)[\s\S]*?loadingIntervalsRef\.current\s*=\s*unregisterLoadingInterval\([\s\S]*?loadingInterval,[\s\S]*?if\s*\(mountedRef\.current\)[\s\S]*?setBusyMatchId\(\(current\)\s*=>\s*clearBusyMatch\(current,\s*requestMatchId\)\)/,
  'each request must clear and unregister its interval before guarded busy cleanup',
);
assert.match(
  componentSource,
  /return\s*\(\)\s*=>\s*\{[\s\S]*?mountedRef\.current\s*=\s*false[\s\S]*?for\s*\(const loadingInterval of loadingIntervalsRef\.current\)[\s\S]*?clearInterval\(loadingInterval\)[\s\S]*?loadingIntervalsRef\.current\.clear\(\)/,
  'unmount must prevent updates and clear every registered interval',
);
assert.match(
  componentSource,
  /setInterval\(\(\)\s*=>\s*\{\s*if\s*\(!mountedRef\.current\)\s*\{\s*return;\s*\}/,
  'loading callbacks must not update state after unmount',
);
assert.match(componentSource, /aria-live=\x22polite\x22/);
assert.match(componentSource, /visuallyHidden/);
assert.match(componentSource, /recoverable\s*&&[\s\S]*?>重试<\/button>/);
assert.doesNotMatch(componentSource, /本场封号|AI 生成|换一个|重新生成/);

console.log('AI match roast UI contracts passed');
