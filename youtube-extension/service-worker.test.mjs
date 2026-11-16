import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import vm from 'node:vm';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
const { Window } = await import(pathToFileURL(require.resolve('happy-dom', { paths: [`${process.cwd()}/reference/bdl/apps/desktop`] })).href);
const source = await fs.readFile(new URL('service-worker.js', import.meta.url), 'utf8');
const channelId = 'UCabcdefghijklmnopqrstuv';

function fixture() {
  const page = new Window();
  page.document.body.innerHTML = '<ytcp-video-title><div id="textbox" contenteditable="true"></div></ytcp-video-title><ytcp-video-description><div id="textbox" contenteditable="true"></div></ytcp-video-description><button id="publish">Publish</button>';
  page.ytInitialData = { metadata: { channelMetadataRenderer: { externalId: channelId } } };
  const saved = {};
  const commands = [];
  const requests = [];
  const tab = { id: 17, url: 'https://www.youtube.com/@deqingdao/videos', status: 'complete' };
  const metadata = { jobId: 'job-1', videoPath: 'D:\\video files\\english.mp4', title: 'A detailed look at the performance', description: 'A short, accurate description.\nSecond line.' };
  const chrome = {
    action: { setBadgeText: async () => {}, setBadgeBackgroundColor: async () => {} },
    runtime: { onMessage: { addListener: () => {} } },
    storage: { session: { set: async (value) => Object.assign(saved, value), get: async (key) => ({ [key]: saved[key] }), remove: async (key) => { delete saved[key]; } } },
    tabs: { get: async () => ({ ...tab }), update: async (_id, value) => Object.assign(tab, value), onUpdated: { addListener: () => {} }, onRemoved: { addListener: () => {} } },
    scripting: { executeScript: async ({ func, args = [] }) => [{ result: vm.runInNewContext(`(${func.toString()})(...args)`, { window: page, document: page.document, Event: page.Event, InputEvent: page.InputEvent, args }) }] },
    debugger: {
      attach: async () => commands.push({ method: 'attach' }),
      detach: async () => commands.push({ method: 'detach' }),
      sendCommand: async (_target, method, params) => {
        commands.push({ method, params });
        if (method === 'DOM.getDocument') return { root: { children: [{ nodeName: 'INPUT', nodeId: 5, attributes: ['type', 'file'] }] } };
        if (method === 'DOM.setFileInputFiles') return {};
        throw new Error('Unexpected browser command: ' + method);
      },
    },
  };
  const context = vm.createContext({ chrome, URL, console, setTimeout, fetch: async (url, request) => {
    const body = JSON.parse(request.body); requests.push({ url, body });
    return { ok: body.token === 'token-1', status: 401, json: async () => url.endsWith('/claim') ? metadata : { accepted: true } };
  } });
  vm.runInContext(source, context);
  return { page, tab, commands, requests, context, saved, metadata };
}

test('uses the existing tab, selects one file and fills real editable fields without publishing', async () => {
  const f = fixture();
  await vm.runInContext("startFromChannel({jobId:'job-1',token:'token-1',port:43891},{tab:{id:17,url:'https://www.youtube.com/@deqingdao/videos'}})", f.context);
  assert.equal(f.tab.url, `https://studio.youtube.com/channel/${channelId}/videos/upload`);
  assert.equal(f.commands.length, 0);
  await vm.runInContext('resume(17)', f.context);
  assert.equal(f.commands.filter((command) => command.method === 'DOM.setFileInputFiles').length, 1);
  assert.equal(f.commands.find((command) => command.method === 'DOM.setFileInputFiles').params.files[0], f.metadata.videoPath);
  assert.equal(f.page.document.querySelector('ytcp-video-title #textbox').textContent, f.metadata.title);
  assert.equal(f.page.document.querySelector('ytcp-video-description #textbox').textContent, f.metadata.description);
  assert.equal(f.requests.at(-1).body.status, 'filled');
  assert.equal(f.saved['cutflow:17'].phase, 'filled');
  await vm.runInContext('resume(17)', f.context);
  assert.equal(f.commands.filter((command) => command.method === 'DOM.setFileInputFiles').length, 1);
  assert.ok(f.commands.every((command) => ['attach', 'detach', 'DOM.getDocument', 'DOM.setFileInputFiles'].includes(command.method)));
});

test('refuses file selection when Studio has switched to another channel', async () => {
  const f = fixture();
  await vm.runInContext("startFromChannel({jobId:'job-1',token:'token-1',port:43891},{tab:{id:17,url:'https://www.youtube.com/@deqingdao/videos'}})", f.context);
  f.tab.url = 'https://studio.youtube.com/channel/UCxxxxxxxxxxxxxxxxxxxxxx/videos/upload';
  await vm.runInContext('resume(17)', f.context);
  assert.equal(f.commands.length, 0);
  assert.equal(f.requests.at(-1).body.status, 'failed');
});

test('keeps source text as literal text rather than executing markup', async () => {
  const f = fixture();
  vm.runInContext('globalThis.fill = fillMetadata', f.context);
  const injected = '<img src=x onerror="window.wasExecuted=true">';
  vm.runInNewContext(`(${f.context.fill.toString()})(title, description)`, { title: 'Literal title', description: injected, document: f.page.document, Event: f.page.Event, InputEvent: f.page.InputEvent });
  assert.equal(f.page.document.querySelector('ytcp-video-description #textbox').textContent, injected);
  assert.equal(f.page.document.querySelector('img'), null);
  assert.equal(f.page.wasExecuted, undefined);
});
