const running = new Set();

async function bridge(job, path, body = {}) {
  const response = await fetch(`http://127.0.0.1:${job.port}${path}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ ...body, jobId: job.jobId, token: job.token }),
  });
  if (!response.ok) throw new Error(`CutFlow bridge returned ${response.status}`);
  return response.json();
}
const key = (tabId) => `cutflow:${tabId}`;
const persist = (job) => chrome.storage.session.set({ [key(job.tabId)]: job });
const restore = async (tabId) => (await chrome.storage.session.get(key(tabId)))[key(tabId)] || null;
const pause = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
async function report(tabId, text) {
  await chrome.action.setBadgeText({ tabId, text }).catch(() => {});
  await chrome.action.setBadgeBackgroundColor({ tabId, color: '#087f6b' }).catch(() => {});
}
const channelIdPattern = /^UC[A-Za-z0-9_-]{22}$/;

async function startFromChannel(message, sender) {
  const tab = sender.tab;
  const url = new URL(tab?.url || 'https://invalid.example');
  if (!tab?.id || url.origin !== 'https://www.youtube.com' || url.pathname !== '/@deqingdao/videos') throw new Error('请从 CutFlow 打开的 @deqingdao 频道页面开始');
  if (running.has(tab.id)) return;
  running.add(tab.id);
  const job = { jobId: message.jobId, token: message.token, port: message.port, tabId: tab.id, phase: 'pending' };
  try {
    if (!Number.isInteger(job.port) || job.port < 1 || job.port > 65535) throw new Error('本地端口无效');
    Object.assign(job, await bridge(job, '/claim'));
    let channelId = null;
    for (let attempt = 0; attempt < 60 && !channelIdPattern.test(channelId || ''); attempt += 1) {
      const results = await chrome.scripting.executeScript({ target: { tabId: tab.id }, world: 'MAIN', func: () => window.ytInitialData?.metadata?.channelMetadataRenderer?.externalId || document.querySelector('meta[itemprop="channelId"]')?.content || document.querySelector('link[rel="canonical"]')?.href?.match(/\/channel\/(UC[A-Za-z0-9_-]{22})/)?.[1] || null });
      channelId = results?.[0]?.result;
      if (!channelIdPattern.test(channelId || '')) await pause(500);
    }
    if (!channelIdPattern.test(channelId || '')) throw new Error('无法核实目标频道 ID，未上传视频');
    job.channelId = channelId; await persist(job);
    await chrome.tabs.update(tab.id, { url: `https://studio.youtube.com/channel/${channelId}/videos/upload` });
    await report(tab.id, '准备');
  } catch (error) {
    await bridge(job, '/complete', { status: 'failed', message: String(error) }).catch(() => {});
    await report(tab.id, '失败'); console.error('[CutFlow YouTube]', error);
  } finally { running.delete(tab.id); }
}

function collectFileNode(node) {
  if (!node) return null;
  if (node.nodeName === 'INPUT' && node.attributes?.some((item, index) => item === 'type' && node.attributes[index + 1] === 'file')) return node.nodeId;
  for (const child of [...(node.children || []), ...(node.shadowRoots || [])]) { const found = collectFileNode(child); if (found) return found; }
  return null;
}

function fillMetadata(nextTitle, nextDescription) {
  const find = (selector, root = document) => {
    const direct = root.querySelector(selector); if (direct) return direct;
    for (const node of root.querySelectorAll('*')) { if (node.shadowRoot) { const nested = find(selector, node.shadowRoot); if (nested) return nested; } }
    return null;
  };
  const title = find('ytcp-video-title #textbox[contenteditable="true"]') || find('#title-textarea #textbox[contenteditable="true"]');
  const description = find('ytcp-video-description #textbox[contenteditable="true"]') || find('#description-textarea #textbox[contenteditable="true"]');
  if (!title || !description) return false;
  const write = (element, value) => { if (element.textContent === value) return; element.focus(); element.textContent = value; element.dispatchEvent(new InputEvent('input', { bubbles: true, composed: true, inputType: 'insertText', data: value })); element.dispatchEvent(new Event('change', { bubbles: true, composed: true })); element.blur(); };
  write(title, nextTitle); write(description, nextDescription);
  return title.textContent === nextTitle && description.textContent === nextDescription;
}

async function resume(tabId) {
  if (running.has(tabId)) return;
  const job = await restore(tabId); if (!job || !['pending', 'file-selected'].includes(job.phase)) return;
  const tab = await chrome.tabs.get(tabId); const url = new URL(tab.url || 'https://invalid.example');
  if (url.origin !== 'https://studio.youtube.com') { await report(tabId, '登录'); return; }
  const currentChannel = url.pathname.match(/^\/channel\/([^/]+)/)?.[1];
  if (!currentChannel) return;
  if (currentChannel !== job.channelId) { await bridge(job, '/complete', { status: 'failed', message: 'Studio 当前频道与 @deqingdao 不一致，未选择视频。' }); await report(tabId, '频道'); return; }
  if (job.phase === 'pending' && !url.pathname.endsWith('/videos/upload')) { await chrome.tabs.update(tabId, { url: `https://studio.youtube.com/channel/${job.channelId}/videos/upload` }); return; }
  running.add(tabId); const debuggee = { tabId }; let attached = false;
  try {
    await chrome.debugger.attach(debuggee, '1.3'); attached = true;
    if (job.phase === 'pending') {
      let nodeId = null;
      for (let attempt = 0; attempt < 90 && !nodeId; attempt += 1) { const document = await chrome.debugger.sendCommand(debuggee, 'DOM.getDocument', { depth: -1, pierce: true }); nodeId = collectFileNode(document.root); if (!nodeId) await pause(1000); }
      if (!nodeId) throw new Error('上传控件未出现，请确认 Studio 已登录且具备目标频道上传权限');
      const current = await chrome.tabs.get(tabId); if (!new URL(current.url).pathname.startsWith(`/channel/${job.channelId}/`)) throw new Error('频道已变化，未选择视频');
      job.phase = 'file-selected'; await persist(job);
      await chrome.debugger.sendCommand(debuggee, 'DOM.setFileInputFiles', { nodeId, files: [job.videoPath.replace(/^\\\\\?\\/, '')] });
    }
    await report(tabId, '填写'); let filled = false;
    for (let attempt = 0; attempt < 120 && !filled; attempt += 1) { const results = await chrome.scripting.executeScript({ target: { tabId }, args: [job.title, job.description], func: fillMetadata }); filled = results?.[0]?.result === true; if (!filled) await pause(1000); }
    if (!filled) throw new Error('标题或简介控件未出现；请先在 Studio 核查，避免重复上传');
    job.phase = 'filled'; await persist(job); await bridge(job, '/complete', { status: 'filled' }); await report(tabId, '审核');
  } catch (error) { job.phase = 'failed'; await persist(job); await bridge(job, '/complete', { status: 'failed', message: String(error) }).catch(() => {}); await report(tabId, '失败'); console.error('[CutFlow YouTube]', error); }
  finally { if (attached) await chrome.debugger.detach(debuggee).catch(() => {}); running.delete(tabId); }
}

chrome.runtime.onMessage.addListener((message, sender) => { if (message?.type === 'cutflow-upload' && sender.tab?.id) startFromChannel(message, sender).catch((error) => console.error('[CutFlow YouTube]', error)); });
chrome.tabs.onUpdated.addListener((tabId, change) => { if (change.status === 'complete') resume(tabId).catch((error) => console.error('[CutFlow YouTube]', error)); });
chrome.tabs.onRemoved.addListener((tabId) => { chrome.storage.session.remove(key(tabId)).catch(() => {}); });
