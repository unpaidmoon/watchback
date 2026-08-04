import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  UpdateAnnouncementDialogView,
  handleUpdateDialogKeyDown,
  restoreUpdateDialogFocus,
} from "../src/features/update/UpdateAnnouncementDialog";

const matches = assert.match;

const announcement = {
  latestVersion: "0.5.3",
  publishedAt: "2026-08-10",
  title: "Watchback <script>alert(1)</script>",
  notes: ["新增程序内更新公告", "修复 <img src=x onerror=alert(1)>"],
};

const html = renderToStaticMarkup(createElement(UpdateAnnouncementDialogView, {
  announcement,
  currentVersion: "0.5.2",
  openingSources: new Set(["baidu"]),
  error: "百度网盘暂时无法打开",
  rootRef: { current: null },
  dismissButtonRef: { current: null },
  onDismiss: () => undefined,
  onOpenDownload: () => undefined,
}));

matches(html, /role="dialog"/);
matches(html, /aria-modal="true"/);
matches(html, /aria-labelledby="update-announcement-title"/);
matches(html, /aria-describedby="update-announcement-description"/);
matches(html, /当前版本 v0\.5\.2/);
matches(html, /最新版本 v0\.5\.3/);
matches(html, /发布于 2026-08-10/);
matches(html, /GitHub/);
matches(html, /百度网盘/);
matches(html, /夸克网盘/);
matches(html, /稍后更新/);
matches(html, /role="alert"/);
matches(html, /百度网盘暂时无法打开/);
matches(html, /&lt;script&gt;alert\(1\)&lt;\/script&gt;/);
assert.doesNotMatch(html, /<script>|<img src=x/);
assert.doesNotMatch(html, /https:\/\//, "remote content and download URLs must not enter the rendered DOM");
matches(html, /data-source="baidu"[^>]*disabled/);
assert.doesNotMatch(html, /data-source="github"[^>]*disabled/);

let dismissals = 0;
let prevented = 0;
const first = { focus: () => focused.push("first") };
const last = { focus: () => focused.push("last") };
const focused: string[] = [];
handleUpdateDialogKeyDown(
  { key: "Escape", preventDefault: () => { prevented += 1; } },
  [],
  null,
  () => { dismissals += 1; },
);
handleUpdateDialogKeyDown(
  { key: "Tab", shiftKey: true, preventDefault: () => { prevented += 1; } },
  [first, last],
  first,
  () => undefined,
);
handleUpdateDialogKeyDown(
  { key: "Tab", shiftKey: false, preventDefault: () => { prevented += 1; } },
  [first, last],
  last,
  () => undefined,
);
assert.equal(dismissals, 1);
assert.equal(prevented, 3);
assert.deepEqual(focused, ["last", "first"]);

let restored = 0;
restoreUpdateDialogFocus({ isConnected: true, focus: () => { restored += 1; } });
restoreUpdateDialogFocus({ isConnected: false, focus: () => { restored += 10; } });
assert.equal(restored, 1);

console.log("update announcement UI tests passed");
