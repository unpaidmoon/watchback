import assert from 'node:assert/strict';
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

const settingsMarkup = renderToStaticMarkup(createElement(DeepSeekSettings));
assert.match(settingsMarkup, new RegExp('AI \\u9510\\u8bc4'));
assert.match(settingsMarkup, new RegExp('\\u672a\\u914d\\u7f6e'));
assert.match(settingsMarkup, /<label[^>]*for=\x22deepseek-api-key\x22[^>]*>DeepSeek API Key<\/label>/);
assert.match(settingsMarkup, /<input[^>]*id=\x22deepseek-api-key\x22[^>]*type=\x22password\x22/);
assert.match(settingsMarkup, /autoComplete=\x22new-password\x22/);
assert.match(settingsMarkup, /aria-live=\x22polite\x22/);
assert.match(settingsMarkup, new RegExp('<button[^>]*>\\u4fdd\\u5b58\\u5e76\\u9a8c\\u8bc1<\\/button>'));
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
const initialMarkup = renderToStaticMarkup(
  createElement(AiMatchRoast, { matchId: 'match-a', initialRoast: null, detailReady: true }),
);
assert.match(initialMarkup, /class=\x22aiMatchRoast/);
assert.match(initialMarkup, /aria-label=\x22AI 锐评\x22/);
assert.match(initialMarkup, /class=\x22visuallyHidden\x22 aria-live=\x22polite\x22/);
assert.match(initialMarkup, /<button[^>]*>生成 AI 锐评<\/button>/);
assert.doesNotMatch(initialMarkup, /正在生成 AI 锐评/);

const incompleteMarkup = renderToStaticMarkup(
  createElement(AiMatchRoast, { matchId: 'match-b', initialRoast: null, detailReady: false }),
);
assert.match(incompleteMarkup, /<button[^>]*disabled=\x22\x22[^>]*>对局详情完整后可生成锐评<\/button>/);

const savedMarkup = renderToStaticMarkup(
  createElement(AiMatchRoast, {
    matchId: 'match-c',
    initialRoast: {
      award: '逆风定海针',
      commentary: '旧版匿名事实。',
      report: {
        verdict: { title: '准星在线，胜负离线', summary: '我方士兵：76 打出了压制力，但关键回合没能兑现。' },
        decisiveFactors: [
          { title: '输出领先', commentary: '伤害位列全场第一。', claimType: 'direct_fact', evidenceRefs: ['我方士兵：76.stats.damage'] },
          { title: '阵亡代价', commentary: '每次阵亡损失的输出高于队内平均。', claimType: 'statistical_interpretation', evidenceRefs: ['我方士兵：76.derived.damagePerDeath'] },
        ],
        currentPlayer: { title: '你这局', commentary: '枪线有压迫感，目标处理还可更果断。', claimType: 'statistical_interpretation', evidenceRefs: ['我方士兵：76.stats.damage'] },
        teamAndMatchups: [
          { type: 'matchup', title: '枪位对局', commentary: '对空任务没有形成结果。', claimType: 'game_context_inference', evidenceRefs: ['gameKnowledge.heroes.法老之鹰'] },
        ],
        closingRoast: '数据像主角，结局却给你安排了群演席。',
      },
    },
    detailReady: true,
  }),
);
for (const heading of ['本场判词', '胜负手', '你这局', '队友与对位', '终审锐评']) {
  assert.match(savedMarkup, new RegExp(`<h3[^>]*>${heading}<\\/h3>`));
}
assert.ok(savedMarkup.indexOf('输出领先') < savedMarkup.indexOf('阵亡代价'));
assert.match(savedMarkup, /<h4[^>]*>准星在线，胜负离线<\/h4>/);
assert.match(savedMarkup, /数据像主角，结局却给你安排了群演席。/);
assert.doesNotMatch(savedMarkup, /evidenceRefs|claimType|我方士兵：76\.stats\.damage|game_context_inference/);
assert.match(savedMarkup, /<button[^>]*>重新生成<\/button>/);

const legacyMarkup = renderToStaticMarkup(
  createElement(AiMatchRoast, {
    matchId: 'match-legacy',
    initialRoast: { award: '逆风定海针', commentary: '匿名事实一。匿名事实二。', report: null },
    detailReady: true,
  }),
);
assert.match(legacyMarkup, /<h3[^>]*>逆风定海针<\/h3>/);
assert.match(legacyMarkup, /<p[^>]*>匿名事实一。匿名事实二。<\/p>/);

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
assert.deepEqual(classifyAiRoastError('锐评输出达到长度上限，未能完整生成。'), {
  message: '锐评输出未完整生成，请重试',
  recoverable: true,
});
assert.deepEqual(classifyAiRoastError('锐评内容被 DeepSeek 安全规则拦截。'), {
  message: '锐评内容被安全规则拦截，请重试',
  recoverable: true,
});
assert.deepEqual(classifyAiRoastError('DeepSeek 返回的锐评格式无效。'), {
  message: '锐评输出格式异常，请重试',
  recoverable: true,
});
assert.deepEqual(classifyAiRoastError('这场对局暂时没有足够的可核验数据生成 AI 锐评。'), {
  message: '这场对局缺少可核验数据，暂时无法生成锐评',
  recoverable: false,
});
assert.deepEqual(classifyAiRoastError('对局详情尚未完整同步，暂时无法生成 AI 锐评。'), {
  message: '对局详情尚未完整同步，请先更新对局数据',
  recoverable: false,
});
assert.deepEqual(classifyAiRoastError('AI 锐评保存失败，请稍后再试。'), {
  message: '锐评保存失败，请检查数据目录后重试',
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

assert.equal(clearBusyMatch('match-a', 'match-a'), null);
assert.equal(clearBusyMatch('match-b', 'match-a'), 'match-b');

const originalLoadingIntervals = new Set([1]);
const registeredLoadingIntervals = registerLoadingInterval(originalLoadingIntervals, 2);
assert.deepEqual([...originalLoadingIntervals], [1]);
assert.deepEqual([...registeredLoadingIntervals], [1, 2]);
const remainingLoadingIntervals = unregisterLoadingInterval(registeredLoadingIntervals, 1);
assert.deepEqual([...registeredLoadingIntervals], [1, 2]);
assert.deepEqual([...remainingLoadingIntervals], [2]);

console.log('AI match roast UI contracts passed');
