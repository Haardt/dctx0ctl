// Optional documentation UI acceptance. Uses Bun's WebSocket and an existing Chromium.
import {strict as assert} from 'node:assert';
import {mkdtemp, readFile, rm} from 'node:fs/promises';
import {resolve, sep} from 'node:path';

const root = resolve(import.meta.dir, '../target/docs-site');
// GitHub's Ubuntu image can expose a Snap Chromium shim that cannot launch
// in its job environment; prefer the installed native Chrome executable.
const binary = process.env.CHROMIUM_BIN ?? Bun.which('google-chrome') ?? Bun.which('chromium');
if (!binary) throw new Error('Chromium is required for --browser; no browser is downloaded.');
const profile = await mkdtemp(resolve(import.meta.dir, '../target/docs-browser-'));
const server = Bun.serve({hostname: '127.0.0.1', port: 0, fetch: async request => {
    const url = new URL(request.url);
    let pathname = decodeURIComponent(url.pathname);
    if (pathname.startsWith('/dctx0ctl/')) pathname = pathname.slice('/dctx0ctl'.length);
    const path = resolve(root, '.' + pathname + (pathname.endsWith('/') ? 'index.html' : ''));
    if (!path.startsWith(root + sep)) return new Response('Forbidden', {status: 403});
    const file = Bun.file(path);
    return await file.exists() ? new Response(file) : new Response('Not found', {status: 404});
}});
const processHandle = Bun.spawn([binary, '--headless', '--no-sandbox', '--disable-gpu',
    '--disable-dev-shm-usage', '--remote-debugging-port=0', '--user-data-dir=' + profile,
    'about:blank'], {stdout: 'ignore', stderr: 'ignore'});
let socket: WebSocket | undefined;
let sequence = 0;
const pending = new Map<number, {resolve: (value: any) => void, reject: (error: Error) => void, timer: Timer}>();
const checks: string[] = [];
const failures: string[] = [];
let targetId: string | undefined;
let sessionId: string | undefined;
async function until(expression: string) {
    const started = Date.now();
    while (Date.now() - started < 7000) {
        try {
            if (await evaluate('Boolean(' + expression + ')')) return;
        } catch (error) {
            // Document navigation can invalidate a CDP execution context between polls.
            if (!/navigated or closed|Cannot find context|Execution context was destroyed/.test(String(error))) throw error;
        }
        await Bun.sleep(25);
    }
    throw new Error('UI condition timed out: ' + expression);
}
async function evaluate(expression: string) {
    const result = await send('Runtime.evaluate', {expression, awaitPromise: true, returnByValue: true}, sessionId);
    if (result.exceptionDetails) throw new Error(expression + ': ' + JSON.stringify(result.exceptionDetails));
    return result.result.value;
}
function send(method: string, params: Record<string, unknown> = {}, session?: string): Promise<any> {
    const id = ++sequence;
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {pending.delete(id); reject(new Error('Browser timeout: ' + method));}, 10000);
        pending.set(id, {resolve, reject, timer});
        socket!.send(JSON.stringify({id, method, params, sessionId: session}));
    });
}
async function navigate(path: string) {
    await send('Page.navigate', {url: server.url.href + 'dctx0ctl/' + path}, sessionId);
    await until('location.pathname === ' + JSON.stringify('/dctx0ctl/' + path) + ' && document.readyState === "complete"');
    await until('document.fonts.status === "loaded"');
}
try {
    let debuggerUrl = '';
    const start = Date.now();
    while (!debuggerUrl) {
        try {
            const [port, endpoint] = (await readFile(profile + '/DevToolsActivePort', 'utf8')).trim().split('\n');
            debuggerUrl = `ws://127.0.0.1:${port}${endpoint}`;
        } catch {
            if (Date.now() - start > 10000) throw new Error('Chromium failed to start.');
            await Bun.sleep(25);
        }
    }
    socket = new WebSocket(debuggerUrl);
    await new Promise<void>((resolve, reject) => {socket!.onopen = () => resolve(); socket!.onerror = () => reject(new Error('Browser connection failed.'));});
    socket.onmessage = event => {
        const packet = JSON.parse(String(event.data));
        if (packet.method === 'Runtime.exceptionThrown') failures.push(JSON.stringify(packet.params.exceptionDetails));
        if (packet.method === 'Network.responseReceived' && packet.params.response.status >= 400) failures.push(packet.params.response.url);
        const request = pending.get(packet.id);
        if (!request) return;
        clearTimeout(request.timer); pending.delete(packet.id);
        packet.error ? request.reject(new Error(packet.error.message)) : request.resolve(packet.result);
    };
    targetId = (await send('Target.createTarget', {url: 'about:blank'})).targetId;
    sessionId = (await send('Target.attachToTarget', {targetId, flatten: true})).sessionId;
    await send('Page.enable', {}, sessionId);
    await send('Runtime.enable', {}, sessionId);
    await send('Network.enable', {}, sessionId);
    await send('Emulation.setDeviceMetricsOverride', {width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false}, sessionId);
    await navigate('index.html');
    assert.match(await evaluate('document.querySelector("main h1").textContent'), /Make intent.*executable/s);
    assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true);
    checks.push('Desktop landing renders without horizontal overflow under project subpath');
    assert.equal(await evaluate('getComputedStyle(document.querySelector("#mdbook-menu-bar")).display'), 'none');
    assert.equal(await evaluate('getComputedStyle(document.querySelector("a.primary-action")).color'), 'rgb(8, 23, 15)');
    checks.push('Landing has one navigation bar and readable primary actions');
    assert.equal(await evaluate('document.querySelectorAll(".language-dctx0 .hljs-keyword").length > 0'), true);
    checks.push('DCTX0 syntax highlighting is active');
    const screenshot = await send('Page.captureScreenshot', {format: 'png', captureBeyondViewport: true}, sessionId);
    await Bun.write(resolve(root, '../docs-site-desktop.png'), Buffer.from(screenshot.data, 'base64'));
    await evaluate('document.querySelector(".hero-actions a").click()');
    await until('location.pathname.endsWith("getting-started.html")');
    await until('document.querySelector("#mdbook-search-toggle")');
    assert.equal(await evaluate('getComputedStyle(document.querySelector(".sidebar")).display !== "none"'), true);
    checks.push('Landing CTA opens the guide with documentation navigation');
    await evaluate('document.querySelector("#mdbook-search-toggle").click()');
    await evaluate('const input = document.querySelector("#mdbook-searchbar"); input.value = "lifecycle"; input.dispatchEvent(new KeyboardEvent("keyup", {key:"e", bubbles:true}));');
    await until('document.querySelectorAll("#mdbook-searchresults a").length > 0');
    checks.push('Local full-text search returns lifecycle documentation');
    await navigate('docs/tutorial/complete-workshophub.html');
    assert.equal(await evaluate('document.querySelector("main h1").textContent.includes("WorkshopHub")'), true);
    assert.equal(await evaluate('document.querySelectorAll(".language-dctx0 .hljs-keyword").length > 20'), true);
    checks.push('Nested complete specification and local font/assets load correctly');
    await send('Emulation.setDeviceMetricsOverride', {width: 390, height: 844, deviceScaleFactor: 1, mobile: true}, sessionId);
    await navigate('index.html');
    assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true);
    checks.push('Mobile landing fits a 390px viewport');
    const mobile = await send('Page.captureScreenshot', {format: 'png', captureBeyondViewport: true}, sessionId);
    await Bun.write(resolve(root, '../docs-site-mobile.png'), Buffer.from(mobile.data, 'base64'));
    await navigate('docs/getting-started.html');
    await evaluate('document.querySelector("#mdbook-sidebar-toggle").click()');
    await until('document.documentElement.classList.contains("sidebar-visible")');
    checks.push('Mobile documentation sidebar opens');
    await evaluate('document.querySelector("#mdbook-sidebar-toggle").click()');
    await until('!document.documentElement.classList.contains("sidebar-visible")');
    checks.push('Mobile documentation sidebar closes');
    assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true);
    checks.push('Mobile guide fits without page overflow');
    assert.equal(await evaluate('document.querySelector("button.clip-button") !== null'), true);
    checks.push('Code blocks expose copy controls');
    assert.deepEqual(failures, []);
    checks.push('No runtime exceptions or failed asset requests');
    console.log(JSON.stringify({status: 'PASS', checks, screenshots: ['target/docs-site-desktop.png', 'target/docs-site-mobile.png'], memory_scope: 'Coupled runner wait4 peak of the child process/reaped descendants; not collective simultaneous RSS'}));
} finally {
    if (socket?.readyState === WebSocket.OPEN) {
        if (targetId) await send('Target.closeTarget', {targetId}).catch(() => {});
        await send('Browser.close').catch(() => {});
        socket.close();
    }
    for (const request of pending.values()) clearTimeout(request.timer);
    if (processHandle.exitCode === null) processHandle.kill('SIGTERM');
    await processHandle.exited;
    server.stop(true);
    await rm(profile, {recursive: true, force: true});
}
