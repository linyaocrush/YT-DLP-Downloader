import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
import { compileModule } from "svelte/compiler";
import ts from "typescript";

const modules = new Map();
const listeners = [];
let calls = [];
let startQueueImpl = async () => ({ completed: 0, failed: 0, cancelled: true });
const api = new Proxy({}, { get: (_, name) => async (...args) => {
  calls.push([name, ...args]);
  if (name === "getSettings") return modules.get("types").defaultSettings();
  if (name === "settingsWarning") return null;
  if (name === "listCookieFiles") return {folder: "", exists: false, files: []};
  if (name === "startQueue") return startQueueImpl(...args);
  if (name === "loadQueue") return [];
} });

function load(name, file, runes = false) {
  const source = readFileSync(new URL(file, import.meta.url), "utf8");
  const js = ts.transpileModule(source, {compilerOptions: {target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext}}).outputText;
  const compiled = runes ? compileModule(js, {generate: "client", filename: file}).js.code : js;
  const cjs = ts.transpileModule(compiled, {compilerOptions: {target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS}}).outputText;
  const exports = {};
  const require = path => {
    if (path === "./api") return api;
    if (path === "./types") return modules.get("types");
    if (path === "./store.svelte") return modules.get("store");
    if (path === "./importLinks") return modules.get("links");
    if (path === "svelte/internal/client") return {
      state: value => ({value}), get: signal => signal.value,
      set: (signal, value) => { signal.value = value; return value; },
      proxy: value => value,
    };
    throw new Error(`Unexpected module ${path}`);
  };
  new Function("require", "exports", cjs)(require, exports);
  modules.set(name, exports);
  return exports;
}

globalThis.window = {setTimeout, clearTimeout};
load("types", "../src/lib/types.ts");
const {previewLinks} = load("links", "../src/lib/importLinks.ts");
const {SettingsStore, settingsStore} = load("store", "../src/lib/store.svelte.ts", true);
const {DownloadStore} = load("download", "../src/lib/download.svelte.ts", true);

async function test(name, run) {
  calls = [];
  await run();
  listeners.push(name);
  console.log(`PASS ${name}`);
}

await test("Cookie 清空同步保存，任务显式传入无 Cookie", async () => {
  settingsStore.value.CookieFile = "C:/old.txt";
  const store = new DownloadStore();
  await store.refreshCookieListing();
  assert.equal(store.selectedCookiePath, null);
  assert.equal(settingsStore.value.CookieFile, "");
  assert.equal(store.buildRequest("https://example.com/v", null, null, false, null).cookieFile, null);
});

await test("设置输入合并写入，flush 保存最终值", async () => {
  const store = new SettingsStore();
  const first = store.set("ProxyHost", "a");
  const second = store.set("ProxyHost", "abc");
  await store.flush();
  await Promise.all([first, second]);
  const writes = calls.filter(call => call[0] === "updateSettings");
  assert.equal(writes.length, 1);
  assert.deepEqual(writes[0][1], {ProxyHost: "abc"});
});

await test("手动模式缺少选流时不启动后端队列", async () => {
  const store = new DownloadStore();
  store.isManualMode = true;
  store.queue = [store.makeItem("https://example.com/v")];
  await store.downloadQueue();
  assert.equal(calls.some(call => call[0] === "startQueue"), false);
  assert.match(store.statusText, /缺少有效手动选流/);
});

await test("停止整批任务仅提交一次队列且保留剩余项", async () => {
  const store = new DownloadStore();
  store.queue = [store.makeItem("https://example.com/1"), store.makeItem("https://example.com/2")];
  startQueueImpl = async () => {
    store.handleQueueUpdate({jobId: 1, itemId: 1, patch: {state: "Downloading"}});
    store.cancel();
    store.handleQueueUpdate({jobId: 1, itemId: 1, patch: {state: "Cancelled"}});
    return {completed: 0, failed: 0, cancelled: true};
  };
  await store.downloadQueue();
  assert.equal(calls.filter(call => call[0] === "startQueue").length, 1);
  assert.equal(store.queue[0].state, "Cancelled");
  assert.equal(store.queue[1].state, "Pending");
  assert.equal(store.isBusy, false);
});

await test("旧任务进度不会覆盖当前任务", async () => {
  const store = new DownloadStore();
  store.isBusy = true;
  store.activeJobId = 2;
  store.progress = 25;
  store.handleUpdate({jobId: 1, itemId: null, percent: 90});
  assert.equal(store.progress, 25);
});

await test("批量链接预览识别等价 YouTube 链接及无效输入", async () => {
  const result = previewLinks("https://youtu.be/abc\nhttps://www.youtube.com/watch?v=abc\ninvalid", []);
  assert.deepEqual(result.map(item => item.status), ["valid", "duplicate", "invalid"]);
});

await test("播放列表可按原始序号筛选范围且仅加入选中项", async () => {
  const store = new DownloadStore();
  store.queueLoaded = true;
  store.playlistChoice = {
    id: "p", title: "List", search: "", selected: [false, false, false],
    entries: [1, 2, 3].map(index => ({index, title: `Video ${index}`, url: `https://example.com/${index}`})),
  };
  store.selectPlaylistRange(2, 3);
  assert.deepEqual(store.playlistChoice.selected, [false, true, true]);
  store.setPlaylistSearch("Video 3");
  assert.deepEqual(store.filteredPlaylistEntries.map(({entry}) => entry.index), [3]);
  store.setPlaylistSearch("");
  store.confirmPlaylistSelection();
  assert.deepEqual(store.queue.map(item => item.playlistIndex), [2, 3]);
});

console.log(`${listeners.length} frontend regression tests passed`);
