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
} from '../src/features/matches/AiMatchRoast';

const apiSource = readFileSync(new URL('../src/shared/api.ts', import.meta.url), 'utf8');
const typesSource = readFileSync(new URL('../src/shared/types.ts', import.meta.url), 'utf8');
const componentSource = readFileSync(
  new URL('../src/features/matches/AiMatchRoast.tsx', import.meta.url),
  'utf8',
);

assert.match(typesSource, /export type AiMatchRoastView\s*=\s*\{\s*award:\s*string;\s*commentary:\s*string;\s*\}/s);
assert.match(typesSource, /export type DeepSeekKeyStatus\s*=\s*\{\s*state:\s*\x27ready\x27\s*\|\s*\x27missing\x27;\s*message:\s*string;\s*\}/s);
assert.match(typesSource, /export type AiRoastGenerationResult\s*=\s*\{\s*aiRoast:\s*AiMatchRoastView;\s*\}/s);
assert.match(typesSource, /export type MatchDetailView\s*=\s*\{[\s\S]*?aiRoast:\s*AiMatchRoastView\s*\|\s*null;/);

assert.match(apiSource, /invoke<DeepSeekKeyStatus>\(\x22get_deepseek_key_status\x22\)/);
assert.match(apiSource, /invoke<DeepSeekKeyStatus>\(\x22save_deepseek_key\x22,\s*\{\s*apiKey\s*\}\)/s);
assert.match(apiSource, /invoke<AiRoastGenerationResult>\(\x22generate_match_ai_roast\x22,\s*\{\s*matchId\s*\}\)/s);

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
assert.match(
  componentSource,
  /finally\s*\{[\s\S]*?clearInterval\(loadingInterval\)[\s\S]*?setBusyMatchId\(\(current\)\s*=>\s*clearBusyMatch\(current,\s*requestMatchId\)\)/,
  'each request must clear its interval and only its own busy match in finally',
);
assert.match(componentSource, /aria-live=\x22polite\x22/);
assert.match(componentSource, /visuallyHidden/);
assert.match(componentSource, /recoverable\s*&&[\s\S]*?>重试<\/button>/);
assert.doesNotMatch(componentSource, /本场封号|AI 生成|换一个|重新生成/);

console.log('AI match roast UI contracts passed');
