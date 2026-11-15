// Rust 后端全量 API 冒烟：对齐旧版 JS server.js 的全部路由
const BASE = process.env.BASE || 'http://127.0.0.1:8899';
let pass = 0, fail = 0;
const ok = (name, cond, extra = '') => {
  if (cond) { pass++; console.log(`  PASS ${name}`); }
  else { fail++; console.log(`  FAIL ${name} ${extra}`); }
};
const j = async (path, body, method) => {
  const r = await fetch(BASE + path, {
    method: method || (body ? 'POST' : 'GET'),
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await r.text();
  let data; try { data = JSON.parse(text); } catch { data = text; }
  return { status: r.status, data, headers: r.headers };
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

console.log('== 基础 ==');
{
  const r = await j('/api/health');
  ok('health', r.status === 200 && r.data.ok === true);
}
{
  const r = await j('/api/meta');
  ok('meta version', r.status === 200 && r.data.version === '1.1.0', JSON.stringify(r.data));
  ok('meta configDir 持久化路径', typeof r.data.configDir === 'string' && r.data.configDir.length > 0, r.data.configDir);
}
{
  const r = await fetch(BASE + '/demo');
  const html = await r.text();
  ok('demo home', r.status === 200 && html.includes('演示靶场') && html.includes('id="main"'));
  ok('demo headers', r.headers.get('server') === 'WebFix-Demo/1.0' && r.headers.get('x-powered-by') === 'DemoPHP/5.6');
  ok('demo cookie', (r.headers.get('set-cookie') || '').includes('session=demo123'));
}
{
  const about = await (await fetch(BASE + '/demo/about')).text();
  ok('demo about', about.includes('__demoCrash'));
  const contact = await (await fetch(BASE + '/demo/contact')).text();
  ok('demo contact', contact.includes('联系我们'));
  const admin = await (await fetch(BASE + '/demo/admin')).text();
  ok('demo admin 登录墙', admin.includes('管理后台 · 登录'));
  const login = await fetch(BASE + '/demo/login', { method: 'POST', redirect: 'manual' });
  ok('demo login 302', login.status === 302 && (login.headers.get('set-cookie') || '').includes('wf_demo_auth=1'));
  const admin2 = await (await fetch(BASE + '/demo/admin', { headers: { cookie: 'wf_demo_auth=1' } })).text();
  ok('demo admin 已登录', admin2.includes('后台面板（已登录）'));
  const sm = await (await fetch(BASE + '/sitemap.xml')).text();
  ok('sitemap', sm.includes('<loc>') && sm.includes('/demo/admin'));
}

console.log('== 体检 / 爬行 ==');
let scanResult;
{
  const r = await j('/api/scan', { url: BASE + '/demo' });
  scanResult = r.data;
  ok('scan 200', r.status === 200);
  ok('scan findings', Array.isArray(r.data.findings) && r.data.findings.length >= 8, `got ${r.data.findings?.length}`);
  ok('scan summary', r.data.summary?.total === r.data.findings.length);
  ok('scan links', Array.isArray(r.data.links) && r.data.links.length >= 3);
  ok('scan headers 抓取', r.data.headers && Object.keys(r.data.headers).length > 0);
  const ids = r.data.findings.map((f) => f.ruleId);
  ok('scan 含 duplicate-id', ids.includes('ui-duplicate-id'));
  ok('scan 含 inline-handler', ids.includes('sec-inline-handler') || ids.includes('ui-inline-handler'), ids.join(','));
  ok('scan 资源健康检查', ids.includes('func-resource-error'), ids.join(','));
}
{
  const r = await j('/api/routes', { url: BASE + '/demo' });
  ok('routes 发现', r.status === 200 && Array.isArray(r.data.routes) && r.data.routes.length >= 3, `routes=${r.data.routes?.length}`);
  ok('routes 含起始页', r.data.routes.some((u) => u.endsWith('/demo')));
  ok('routes 含子页', r.data.routes.some((u) => u.includes('/demo/about')));
  const noauth = await j('/api/routes', { url: 'https://example.com/' });
  ok('routes 未授权拦截', noauth.status === 400);
}
{
  const r = await j('/api/scan', { url: 'https://example.com/' });
  ok('scan 未授权拦截', r.status === 400 && r.data.error.includes('授权'));
  const r2 = await j('/api/scan', {});
  ok('scan 缺 url', r2.status === 400);
  const r3 = await j('/api/scan', { url: 'not a url' });
  ok('scan 坏 url', r3.status === 400);
}
let crawlId;
{
  const r = await j('/api/crawl', { url: BASE + '/demo', maxPages: 10, maxDepth: 2 });
  crawlId = r.data.id;
  ok('crawl 启动', r.status === 200 && !!crawlId);
  let job;
  for (let i = 0; i < 60; i++) {
    await sleep(500);
    job = (await j('/api/crawl-status?id=' + crawlId)).data;
    if (job.status !== 'running') break;
  }
  ok('crawl 完成', job.status === 'done', JSON.stringify(job).slice(0, 200));
  ok('crawl 多页', job.pages.length >= 3, `pages=${job.pages?.length}`);
  ok('crawl 聚合 findings', job.findings.length >= 8);
  ok('crawl summary', job.summary?.total === job.findings.length);
  ok('crawl 逐页 findings', job.pages.every((p) => Array.isArray(p.findings) && p.count === p.findings.length), JSON.stringify(job.pages[0]).slice(0, 200));
  ok('crawl 逐页有检出', job.pages.some((p) => p.findings.length > 0));
  const missing = await j('/api/crawl-status?id=nope');
  ok('crawl 404', missing.status === 404);
}

console.log('== 人工巡检 / 代理 / 抓取 ==');
let sid, token, cookieName, cookiePath;
{
  const r = await j('/api/inspect/start', { url: BASE + '/demo' });
  ({ sid, token, cookieName, cookiePath } = r.data);
  ok('inspect start', r.status === 200 && !!sid && !!token && r.data.proxyUrl.startsWith('/proxy/'));
  ok('inspect cookie 字段', cookieName.endsWith('sid') && cookiePath === '/proxy/' + token);
  const page = await fetch(BASE + r.data.proxyUrl, { headers: { cookie: `${cookieName}=${sid}` } });
  const html = await page.text();
  ok('proxy 页面', page.status === 200 && html.includes('演示靶场'));
  ok('proxy 注入 hook', html.includes('/capture-hook.js?'));
  ok('proxy 重写链接', html.includes('/proxy/'));
  const hook = await fetch(BASE + '/capture-hook.js');
  ok('capture-hook.js 静态', hook.status === 200 && (await hook.text()).includes('WebFix'));
  const pilot = await fetch(BASE + '/ai-pilot.js');
  ok('ai-pilot.js 静态', pilot.status === 200 && (await pilot.text()).includes('WfPilot'));
}
{
  const r = await j('/api/capture', { sid, type: 'console', level: 'error', message: '冒烟测试错误', pageUrl: BASE + '/proxy/' + token + '/demo' });
  ok('capture 写入', r.status === 200 && r.data.ok);
  const cap = await j('/api/captures?sid=' + sid);
  const mine = cap.data.events.find((e) => e.message === '冒烟测试错误');
  ok('captures 读取', cap.status === 200 && !!mine, `events=${cap.data.events?.length}`);
  ok('captures realUrl', mine && mine.realUrl.includes('/demo'), mine?.realUrl);
  ok('captures target', cap.data.target === BASE + '/demo');
  const clear = await j('/api/inspect/clear', { sid });
  ok('inspect clear', clear.data.ok);
  const cap2 = await j('/api/captures?sid=' + sid);
  ok('clear 后为空', cap2.data.events.length === 0);
  const bad = await j('/api/captures?sid=nope');
  ok('captures 404', bad.status === 404);
}
{
  const r = await j('/api/scan-html', { html: '<html><body><img src="x.png"><p id="a"></p><p id="a"></p></body></html>', url: 'http://localhost/' });
  ok('scan-html', r.status === 200 && r.data.findings.some((f) => f.ruleId === 'ui-img-no-alt') && r.data.findings.some((f) => f.ruleId === 'ui-duplicate-id'));
}

console.log('== 修复四件套 ==');
{
  const finding = { ruleId: 'sec-sqli', title: 'SQL 注入', url: 'http://x/', severity: 'critical', category: 'security', evidence: '参数拼接', steps: '输入引号' };
  const r = await j('/api/fix', { finding, stack: { frontend: 'React', backend: 'Rust' } });
  const d = r.data.deliverables;
  ok('fix 四件套', r.status === 200 && d.minimalFix.frontend && d.minimalFix.backend && d.verifySteps.length && d.guardCode && d.gitDiff);
  ok('fix prompt 精简', r.data.prompt.includes('【问题】') && r.data.prompt.includes('【修复】'));
  ok('fix fullPrompt 七字段', r.data.fullPrompt.includes('【BUG标题】') && r.data.fullPrompt.includes('【根因分析】'));
  const p = await j('/api/prompt', { fields: finding, style: 'brief' });
  ok('prompt brief', p.data.prompt.split('\n').length <= 8);
  const p2 = await j('/api/prompt', { fields: finding, style: 'full', stack: {} });
  ok('prompt full', p2.data.prompt.includes('【技术栈】'));
  const patch = await j('/api/patch', { items: [{ title: 'T', severity: 'high', gitDiff: 'diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n' }] });
  ok('patch 文本', patch.status === 200 && String(patch.data).includes('diff --git'));
  const report = await j('/api/report', { finding, stack: {} });
  ok('report markdown', report.status === 200 && String(report.data).includes('#'));
}

console.log('== AI（零成本默认）==');
{
  // 配置会持久化到系统用户目录：先复位为零成本默认，保证断言不受历史配置影响
  await j('/api/ai/config', { enabled: false });
  const r = await j('/api/ai/config');
  ok('ai config', r.status === 200 && r.data.providers.length === 8 && r.data.config.enabled === false);
  const bad = await j('/api/ai/config', { baseUrl: 'ftp://x' });
  ok('ai baseUrl 校验', bad.status === 400);
  const save = await j('/api/ai/config', { provider: 'ollama', baseUrl: 'http://127.0.0.1:11434/v1', model: 'qwen2.5:7b' });
  ok('ai 保存', save.status === 200 && save.data.config.provider === 'ollama');
  const ana = await j('/api/ai/analyze', { kind: 'scan', findings: [] });
  ok('ai 未启用拒绝（零成本）', ana.status === 502 && ana.data.error.includes('启用'), JSON.stringify(ana.data));
  const test = await j('/api/ai/test', {});
  ok('ai test 无服务 502', test.status === 502);
  await j('/api/ai/config', { provider: 'deepseek', baseUrl: '', model: '' });
}

console.log('== 扩展中心 ==');
let extId;
{
  const meta = await j('/api/ext/meta');
  ok('ext meta', meta.data.types.length === 4 && meta.data.transports.length === 6 && meta.data.modes.length === 3 && meta.data.ops.length === 9);
  const list = await j('/api/ext/list');
  ok('ext 内置 8 个', list.data.extensions.filter((e) => e.builtin).length === 8);
  const reg = await j('/api/ext/register', { type: 'skill', transport: 'builtin', name: '冒烟技能' });
  ok('ext 注册 builtin 拒绝', reg.status === 400);
  const reg2 = await j('/api/ext/register', { type: 'bot', transport: 'http', name: '冒烟Bot', endpoint: 'http://127.0.0.1:9/none', permissions: ['notify'] });
  extId = reg2.data.ext?.id;
  ok('ext 注册', reg2.status === 200 && !!extId);
  const upd = await j('/api/ext/update', { id: extId, name: '改名Bot', enabled: false });
  ok('ext 更新', upd.status === 200 && upd.data.ext.name === '改名Bot' && upd.data.ext.enabled === false);
  const inv = await j('/api/ext/invoke', { id: 'skill-header', input: { url: BASE + '/demo' }, ctx: {}, authorized: true });
  ok('ext invoke builtin', inv.status === 200 && inv.data.ok === true, JSON.stringify(inv.data).slice(0, 150));
  const invAuth = await j('/api/ext/invoke', { id: 'agent-rule', input: { url: 'https://example.com/' } });
  ok('ext invoke 授权拦截', invAuth.status === 400);
  const disc = await j('/api/ext/discover', { id: 'skill-header' });
  ok('ext discover', disc.status === 200);
  const audit = await j('/api/audit?limit=50');
  ok('audit 有记录', audit.data.entries.length > 0);
  const rm = await j('/api/ext/remove', { id: extId });
  ok('ext 删除', rm.status === 200);
  const rmBuiltin = await j('/api/ext/remove', { id: 'skill-header' });
  ok('ext 内置保护', rmBuiltin.status === 400);
}

console.log('== 扩展市场 / AI 添加 / 拖拽导入 ==');
{
  const ps = await j('/api/ext/presets');
  ok('presets 列表', ps.status === 200 && ps.data.presets.length >= 12);
  ok('presets 含 nuclei', ps.data.presets.some((p) => p.id === 'tool-nuclei' && p.transport === 'cli'));
  ok('presets 含 module 技能', ps.data.presets.some((p) => p.id === 'skill-jwt' && p.transport === 'module'));
  // 扩展商店也会持久化：先清理可能残留的同名预设，保证安装断言可重复
  await j('/api/ext/remove', { id: 'skill-jwt' });
  await j('/api/ext/remove', { id: 'tool-nuclei' });
  // 一键接入内置 module 技能（零成本开箱即用）
  const inst = await j('/api/ext/install-preset', { id: 'skill-jwt' });
  ok('install module 预设', inst.status === 200 && inst.data.ext.id === 'skill-jwt' && inst.data.ext.transport === 'module', JSON.stringify(inst.data).slice(0, 150));
  const dup = await j('/api/ext/install-preset', { id: 'skill-jwt' });
  ok('install 重复拒绝', dup.status === 400);
  // 调用刚接入的 module 技能（Node 执行，本地演示无需授权）
  const inv = await j('/api/ext/invoke', { id: 'skill-jwt', input: { url: BASE + '/demo', html: '<script>var t="eyJhbGciOiJub25lIn0.eyJzdWIiOiIxIn0.x";</script>' }, ctx: {}, authorized: true });
  ok('module 技能执行', inv.status === 200 && inv.data.ok === true, JSON.stringify(inv.data).slice(0, 200));
  // 接入外部 cli 工具预设（默认禁用）
  const instCli = await j('/api/ext/install-preset', { id: 'tool-nuclei' });
  ok('install cli 预设', instCli.status === 200 && instCli.data.ext.transport === 'cli' && instCli.data.ext.enabled === false);
  // AI 对话添加：命中预设（零成本）
  const ai = await j('/api/ext/ai-compose', { prompt: '帮我接入 nuclei 漏扫' });
  ok('ai-compose 命中预设', ai.status === 200 && ai.data.source === 'preset' && ai.data.draft.id === 'tool-nuclei');
  const aiEmpty = await j('/api/ext/ai-compose', { prompt: '' });
  ok('ai-compose 空 prompt 拒绝', aiEmpty.status === 502);
  // 拖拽导入：.mjs 技能
  const imp = await j('/api/ext/import', { filename: 'smoke-skill.mjs', content: 'export default function(){return {output:{ok:1},findings:[]};}' });
  ok('import mjs 技能', imp.status === 200 && imp.data.kind === 'module' && imp.data.ext.transport === 'module', JSON.stringify(imp.data).slice(0, 150));
  const impInv = await j('/api/ext/invoke', { id: imp.data.ext.id, input: { url: BASE + '/demo' }, ctx: {}, authorized: true });
  ok('import 技能可执行', impInv.status === 200 && impInv.data.ok === true);
  // 拖拽导入：.json MCP 配置
  const impJson = await j('/api/ext/import', { filename: 'mcp.json', content: JSON.stringify({ mcpServers: { smoke: { command: 'node', args: ['x.mjs'] } } }) });
  ok('import json MCP', impJson.status === 200 && impJson.data.ext.type === 'mcp' && impJson.data.ext.transport === 'stdio');
  const impBad = await j('/api/ext/import', { filename: 'x.txt', content: 'hi' });
  ok('import 不支持类型拒绝', impBad.status === 400);
  // 清理导入的扩展
  await j('/api/ext/remove', { id: 'skill-jwt' });
  await j('/api/ext/remove', { id: 'tool-nuclei' });
  await j('/api/ext/remove', { id: imp.data.ext.id });
  await j('/api/ext/remove', { id: impJson.data.ext.id });
}

console.log('== 协同编排 ==');
let pipeId, runId;
{
  const pipes = await j('/api/orch/pipelines');
  ok('orch 内置 pipe-standard', pipes.data.pipelines.some((p) => p.id === 'pipe-standard' && p.builtin));
  const save = await j('/api/orch/pipeline', {
    name: '冒烟编排', mode: 'serial',
    steps: [
      { key: 's1', ext: 'agent-rule', input: { url: BASE + '/demo' } },
      { key: 's2', ext: 'bot-report', input: {}, when: { path: 'stats.total', op: 'gte', value: 0 } },
      { key: 's3', ext: 'bot-report', input: {}, when: { path: 'stats.total', op: 'lt', value: 0 } },
    ],
  });
  pipeId = save.data.pipeline?.id;
  ok('orch 保存', save.status === 200 && !!pipeId);
  const badMode = await j('/api/orch/pipeline', { mode: 'xxx', steps: [{ ext: 'a' }] });
  ok('orch 校验 mode', badMode.status === 400);
  const run = await j('/api/orch/run', { pipelineId: pipeId, url: BASE + '/demo', authorized: true });
  runId = run.data.id;
  ok('orch 运行', run.status === 200 && !!runId);
  let st;
  for (let i = 0; i < 40; i++) {
    await sleep(300);
    st = (await j('/api/orch/run-status?id=' + runId)).data;
    if (st.status !== 'running') break;
  }
  ok('orch 完成', st.status === 'done', JSON.stringify(st).slice(0, 200));
  ok('orch 步骤 3', st.steps.length === 3);
  ok('orch 条件跳过', st.steps[2].status === 'skipped' && st.steps[1].status === 'ok');
  ok('orch findings 聚合', st.stats.total >= 1);
  const runAuth = await j('/api/orch/run', { pipelineId: pipeId, url: 'https://example.com/' });
  ok('orch 授权拦截', runAuth.status === 400);
  const rm = await j('/api/orch/pipeline/remove', { id: pipeId });
  ok('orch 删除', rm.status === 200);
  const rmB = await j('/api/orch/pipeline/remove', { id: 'pipe-standard' });
  ok('orch 内置保护', rmB.status === 400);
  // AI 自动编排（零成本规则模式）
  const aiPipe = await j('/api/orch/ai-compose', { prompt: '扫描全站并挖掘秘钥泄露，最后汇总报告' });
  ok('orch ai-compose', aiPipe.status === 200 && Array.isArray(aiPipe.data.pipeline?.steps) && aiPipe.data.pipeline.steps.length >= 2, JSON.stringify(aiPipe.data).slice(0, 200));
  ok('orch ai-compose 含 report', aiPipe.data.pipeline.steps.some((s) => s.ext === 'bot-report'));
  const aiEmpty = await j('/api/orch/ai-compose', { prompt: '' });
  ok('orch ai-compose 空拒绝', aiEmpty.status === 502);
  const st404 = await j('/api/orch/run-status?id=nope');
  ok('orch run 404', st404.status === 404);
}

console.log('== 巡检登录态复用（登录墙） ==');
{
  const s = await j('/api/inspect/start', { url: BASE + '/demo', authorized: true });
  const token = s.data.token, sid = s.data.sid, cookieName = s.data.cookieName;
  ok('登录态 会话建立', s.status === 200 && !!token && !!sid);
  const login = await fetch(BASE + '/proxy/' + token + '/demo/login', {
    method: 'POST', redirect: 'manual',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded', Cookie: `${cookieName}=${sid}` },
    body: 'user=demo&pwd=demo',
  });
  const sc = login.headers.getSetCookie ? login.headers.getSetCookie() : [login.headers.get('set-cookie')];
  const authPair = ((sc || []).find((c) => c && c.includes('wf_demo_auth')) || '').split(';')[0];
  ok('登录态 代理登录回写', !!authPair);
  await fetch(BASE + '/proxy/' + token + '/demo/admin', { headers: { Cookie: `${cookieName}=${sid}; ${authPair}` } });
  const hasEval = (d) => (d.findings || []).some((f) => f.ruleId === 'sec-eval-dom');
  const withS = await j('/api/scan', { url: BASE + '/demo/admin', authorized: true, useSession: true });
  const noS = await j('/api/scan', { url: BASE + '/demo/admin', authorized: true, useSession: false });
  const manual = await j('/api/scan', { url: BASE + '/demo/admin', authorized: true, useSession: false, cookie: 'wf_demo_auth=1' });
  ok('登录态 复用扫到后台', withS.status === 200 && hasEval(withS.data));
  ok('登录态 不复用仅登录页', noS.status === 200 && !hasEval(noS.data));
  ok('登录态 手动Cookie兜底', manual.status === 200 && hasEval(manual.data));
  const cr = await j('/api/crawl', { url: BASE + '/demo', authorized: true, useSession: true, maxPages: 8, maxDepth: 2 });
  let cj = null;
  for (let i = 0; i < 30; i++) { await sleep(700); cj = await j('/api/crawl-status?id=' + cr.data.id); if (cj.data.status !== 'running') break; }
  const ap = (cj.data.pages || []).find((p) => p.url.endsWith('/demo/admin'));
  ok('登录态 爬行覆盖后台页', !!ap);
  ok('登录态 爬行后台逐页检出', !!ap && (ap.findings || []).some((f) => f.ruleId === 'sec-eval-dom'));
}

console.log('== 静态 / 兜底 ==');
{
  const idx = await fetch(BASE + '/');
  ok('首页静态', idx.status === 200 && (await idx.text()).includes('<html'));
  const nf = await j('/no-such-file');
  ok('404 JSON', nf.status === 404 && nf.data.error === 'not found');
  const head = await fetch(BASE + '/capture-hook.js', { method: 'HEAD' });
  ok('HEAD 支持', head.status === 200);
}

console.log(`\n结果：${pass} 通过 / ${fail} 失败`);
process.exit(fail ? 1 : 0);