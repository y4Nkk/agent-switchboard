import assert from "node:assert/strict";
import { test } from "node:test";
import { reduceInformationSources as reduce, type InformationNotice,
  type InformationSourceState } from "../src/components/information-source-state.ts";

const notice = (id: string, title = id): InformationNotice => ({ id, title, kind: "warning" });
const empty = (): InformationSourceState => new Map();

test("删除跨来源卸载、重新挂载与语言切换保留，卸载释放展示内容", () => {
  let state = reduce(empty(), { type: "publish", source: "usage", messages: [notice("issue", "中文")] });
  state = reduce(state, { type: "dismiss", source: "usage", id: "issue" });
  state = reduce(state, { type: "publish", source: "usage", messages: null });
  assert.equal(state.get("usage")?.messages, null);
  assert.equal(state.get("usage")?.statuses.get("issue"), "dismissed");
  state = reduce(state, { type: "publish", source: "usage", messages: [notice("issue", "English")] });
  assert.equal(state.get("usage")?.messages?.[0].title, "English");
  assert.equal(state.get("usage")?.statuses.get("issue"), "dismissed");
});

test("确认跨页面保留，删除后迟到的确认不能复活消息", () => {
  let state = reduce(empty(), { type: "publish", source: "sessions", messages: [notice("issue")] });
  state = reduce(state, { type: "acknowledge", source: "sessions", id: "issue" });
  state = reduce(state, { type: "publish", source: "sessions", messages: null });
  state = reduce(state, { type: "publish", source: "sessions", messages: [notice("issue")] });
  assert.equal(state.get("sessions")?.statuses.get("issue"), "acknowledged");
  state = reduce(state, { type: "dismiss", source: "sessions", id: "issue" });
  assert.equal(reduce(state, { type: "acknowledge", source: "sessions", id: "issue" }), state);
});

test("未知或卸载的来源不证明问题已解决，成功空快照才清理状态并允许复发", () => {
  let state = reduce(empty(), { type: "publish", source: "usage", messages: [notice("issue")] });
  state = reduce(state, { type: "dismiss", source: "usage", id: "issue" });
  state = reduce(state, { type: "publish", source: "usage", messages: null });
  assert.equal(state.get("usage")?.statuses.size, 1);
  state = reduce(state, { type: "publish", source: "usage", messages: [] });
  assert.equal(state.get("usage")?.statuses.size, 0);
  state = reduce(state, { type: "publish", source: "usage", messages: [notice("issue")] });
  assert.equal(state.get("usage")?.statuses.has("issue"), false);
});

test("相同消息身份不跨来源串用，过时动作不能创建幽灵状态", () => {
  let state = reduce(empty(), { type: "publish", source: "usage", messages: [notice("issue")] });
  state = reduce(state, { type: "publish", source: "sessions", messages: [notice("issue")] });
  state = reduce(state, { type: "dismiss", source: "usage", id: "issue" });
  assert.equal(state.get("sessions")?.statuses.size, 0);
  assert.equal(reduce(state, { type: "dismiss", source: "usage", id: "missing" }), state);
  state = reduce(state, { type: "publish", source: "sessions", messages: null });
  assert.equal(state.has("sessions"), false);
  assert.equal(reduce(state, { type: "dismiss", source: "sessions", id: "issue" }), state);
});

test("新问题保持可见，替换快照清理旧身份，不累计删除历史", () => {
  let state = empty();
  for (let index = 0; index < 100; index++) {
    const id = `issue-${index}`;
    state = reduce(state, { type: "publish", source: "sessions", messages: [notice(id)] });
    assert.equal(state.get("sessions")?.statuses.size, 0);
    state = reduce(state, { type: "dismiss", source: "sessions", id });
    assert.equal(state.get("sessions")?.statuses.size, 1);
  }
});

test("当前来源身份去重，新的应用所有者不继承上次运行的决定", () => {
  const messages = [notice("issue", "first"), notice("issue", "current")];
  const previous = reduce(reduce(empty(), { type: "publish", source: "sessions", messages }),
    { type: "dismiss", source: "sessions", id: "issue" });
  const restarted = reduce(empty(), { type: "publish", source: "sessions", messages });
  assert.equal(previous.get("sessions")?.statuses.size, 1);
  assert.equal(restarted.get("sessions")?.statuses.size, 0);
  assert.deepEqual(restarted.get("sessions")?.messages, [notice("issue", "current")]);
});
