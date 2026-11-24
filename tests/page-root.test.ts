import assert from "node:assert/strict";
import { test } from "node:test";
import type { PageRootGuard, PageRootView } from "../src/app/page-root.ts";
import { createPageRoot, joinPageRoot } from "../src/app/page-root.ts";

/** A guard that records the runs it is asked to allow instead of deciding itself. */
function guard(overrides: Partial<PageRootGuard> = {}) {
  const asked: Array<() => void> = [];
  const member = { current: {
    dirty: false, busy: false, request: (run: () => void) => { asked.push(run); }, ...overrides } };
  return { member, asked };
}

/** A page whose second-level views record their names when the return closes them. */
function page() {
  const root = createPageRoot();
  const closed: string[] = [];
  const view = (name: string, overrides: Partial<PageRootView> = {}) => {
    const member = { current: { open: true, close: () => { closed.push(name); }, busy: false, ...overrides } };
    return { member, leave: joinPageRoot(root.views, member) };
  };
  return { root, closed, view };
}

test("the return closes every second-level view when nothing holds it", () => {
  const { root, closed, view } = page();
  view("import");
  view("not shown", { open: false, busy: true });
  view("editor");
  root.returnToRoot();
  assert.deepEqual(closed, ["import", "editor"]);
});

test("a return at the root leaves closed views and unsaved guards alone", () => {
  const { root, closed, view } = page();
  view("editor", { open: false });
  const dirty = guard({ dirty: true });
  joinPageRoot(root.guards, dirty.member);
  root.returnToRoot();
  assert.deepEqual(closed, []);
  assert.deepEqual(dirty.asked, []);
});

test("a view that is saving or scanning holds the return like its disabled back control", () => {
  const { root, closed, view } = page();
  view("import", { busy: true });
  view("editor");
  root.returnToRoot();
  assert.deepEqual(closed, []);
});

test("the latest busyness decides, not the value at registration", () => {
  const { root, closed, view } = page();
  view("editor");
  const scanning = view("import");
  scanning.member.current.busy = true;
  root.returnToRoot();
  assert.deepEqual(closed, []);
  scanning.member.current.busy = false;
  root.returnToRoot();
  assert.deepEqual(closed, ["editor", "import"]);
});

test("a guard without changes lets the return through unasked", () => {
  const { root, closed, view } = page();
  view("history");
  const clean = guard();
  joinPageRoot(root.guards, clean.member);
  root.returnToRoot();
  assert.deepEqual(clean.asked, []);
  assert.deepEqual(closed, ["history"]);
});

test("a guard with unsaved changes asks first and closes only after the discard", () => {
  const { root, closed, view } = page();
  view("editor");
  const dirty = guard({ dirty: true });
  joinPageRoot(root.guards, dirty.member);
  root.returnToRoot();
  assert.deepEqual(closed, []);
  assert.equal(dirty.asked.length, 1);
  dirty.asked[0]();
  assert.deepEqual(closed, ["editor"]);
});

test("the latest dirtiness decides, not the value at registration", () => {
  const { root, closed, view } = page();
  view("editor");
  const late = guard();
  joinPageRoot(root.guards, late.member);
  late.member.current.dirty = true;
  root.returnToRoot();
  assert.equal(late.asked.length, 1);
  assert.deepEqual(closed, []);
});

test("a guard that is saving holds the return even when another view has unsaved changes", () => {
  const { root, closed, view } = page();
  view("editor");
  const saving = guard({ busy: true });
  const dirty = guard({ dirty: true });
  joinPageRoot(root.guards, saving.member);
  joinPageRoot(root.guards, dirty.member);
  root.returnToRoot();
  assert.deepEqual(saving.asked, []);
  assert.deepEqual(dirty.asked, []);
  assert.deepEqual(closed, []);
});

test("among several unsaved views only the first is asked, since one discard closes them all", () => {
  const { root, closed, view } = page();
  view("repository form");
  view("skill editor");
  const first = guard({ dirty: true });
  const second = guard({ dirty: true });
  joinPageRoot(root.guards, first.member);
  joinPageRoot(root.guards, second.member);
  root.returnToRoot();
  assert.equal(first.asked.length, 1);
  assert.deepEqual(second.asked, []);
  first.asked[0]();
  assert.deepEqual(closed, ["repository form", "skill editor"]);
});

test("a view that left no longer holds or guards the return", () => {
  const { root, closed, view } = page();
  view("editor");
  const scanning = view("import", { busy: true });
  const dirty = guard({ dirty: true });
  const leaveGuard = joinPageRoot(root.guards, dirty.member);
  scanning.leave();
  leaveGuard();
  root.returnToRoot();
  assert.deepEqual(dirty.asked, []);
  assert.deepEqual(closed, ["editor"]);
});
