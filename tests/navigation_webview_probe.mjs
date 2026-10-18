// Real WebView2 probe; invoked by check_creation_navigation.py after isolated launch.
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require('../reference/bdl/apps/desktop/node_modules/@playwright/test');
const [endpoint, roundsText = '10'] = process.argv.slice(2);
const rounds = Number(roundsText);
if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(endpoint) || !Number.isInteger(rounds) || rounds < 1 || rounds > 100) {
  throw new Error('Expected loopback CDP endpoint and rounds in 1..100');
}
const browser = await chromium.connectOverCDP(endpoint, { timeout: 10000 });
const pages = browser.contexts().flatMap(context => context.pages());
const page = pages.find(value => !value.url().startsWith('devtools:')) || pages[0];
if (!page) throw new Error('No application WebView page');
const errors = [];
page.on('pageerror', error => errors.push(error.message));
await page.waitForSelector('.side-nav .nav-item', { timeout: 30000 });
const notice = page.getByRole('button', { name: '我已阅读并了解，继续使用', exact: true });
if (await notice.count() && await notice.isVisible()) await notice.click();
const viewport = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio }));
const cdp = await page.context().newCDPSession(page);
const version = await cdp.send('Browser.getVersion');
const tabs = [
  { id: 'creation', label: 'AI 创作', selector: '.creation-studio,.creation-page' },
  { id: 'editor', label: '剪辑', selector: '.editor-launch' },
  { id: 'settings', label: '设置', selector: '.settings-grid' },
  { id: 'about', label: '关于', selector: '.about-header' },
  { id: 'bilibili', label: 'B站素材', selector: '.bilibili-workspace' },
];
async function measure(tab, phase, index) {
  // The clock starts in the browser capture-phase click listener, excluding CDP dispatch latency.
  await page.evaluate(({ selector, id }) => {
    window.__cutflowNavReceipt = new Promise((resolve, reject) => {
      const click = event => {
        const button = event.target.closest('.side-nav .nav-item');
        if (!button) return;
        document.removeEventListener('click', click, true);
        const started = performance.now();
        let first = null, firstKind = null, stable = null, detectedFrame = false;
        const visible = element => {
          if (!element?.getBoundingClientRect().width) return false;
          for (let current = element; current; current = current.parentElement) {
            const css = getComputedStyle(current);
            if (css.display === 'none' || css.visibility === 'hidden' || Number(css.opacity) < 0.01) return false;
            if (current.matches('.main-region')) break;
          }
          return true;
        };
        const opaque = element => {
          if (!visible(element)) return false;
          for (let current = element; current; current = current.parentElement) {
            if (Number(getComputedStyle(current).opacity) < 0.99) return false;
            if (current.matches('.main-region')) break;
          }
          return true;
        };
        const frame = () => {
          const root = document.querySelector('.main-region[data-page="' + id + '"]');
          const content = root?.querySelector(selector);
          const skeleton = root?.querySelector('.workspace-loading');
          if (first === null && (visible(content) || visible(skeleton))) {
            if (detectedFrame) first = performance.now() - started;
            else { detectedFrame = true; firstKind = visible(content) ? 'content' : 'skeleton'; }
          }
          if (opaque(content)) stable = performance.now() - started;
          if (first !== null && stable !== null) {
            requestAnimationFrame(() => resolve({ firstFrameMs: first, settledMs: stable, firstKind }));
          } else if (performance.now() - started > 15000) {
            reject(new Error('Target page did not render: ' + id));
          } else requestAnimationFrame(frame);
        };
        requestAnimationFrame(frame);
      };
      document.addEventListener('click', click, true);
    });
  }, tab);
  await page.locator('.side-nav .nav-item[aria-label="' + tab.label + '"]').click();
  const timing = await page.evaluate(() => window.__cutflowNavReceipt);
  return { phase, index, tab: tab.id, ...timing };
}
const measurements = [];
for (const [index, tab] of tabs.slice(0, 4).entries()) measurements.push(await measure(tab, 'cold', index));
await measure(tabs[4], 'prewarm', 0);
for (let round = 0; round < rounds; round++) {
  for (const tab of tabs) measurements.push(await measure(tab, 'warm', round));
}
const rendered = await page.evaluate(() => ({
  domNodes: document.querySelectorAll('*').length,
  activeTab: document.querySelector('.main-region')?.getAttribute('data-page'),
}));
console.log(JSON.stringify({ version, viewport, rendered, errors, measurements }));
// This process only owns its CDP connection. The Python parent owns application shutdown.
process.exit(errors.length ? 2 : 0);
