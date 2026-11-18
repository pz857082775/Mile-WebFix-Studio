import http from 'node:http';
import { DatabaseSync } from 'node:sqlite';

const host = '127.0.0.1';
const port = Number(process.env.DEMO_PORT || 8899);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('DEMO_PORT 必须是有效端口');
const db = new DatabaseSync(':memory:');
db.exec(`
  CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT, password TEXT, role TEXT, email TEXT);
  INSERT INTO users VALUES (1,'alice','demo123','user','alice@example.invalid'),(2,'bob','demo123','user','bob@example.invalid'),(3,'admin','admin123','admin','admin@example.invalid');
  CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, price INTEGER);
  INSERT INTO products VALUES (1,'鼠标',39),(2,'键盘',99),(3,'显示器',799);
`);
const messages = [];
const uploads = [];
const nav = `<nav><a href="/">首页</a><a href="/login">登录</a><a href="/users">用户管理</a><a href="/products">数据列表</a><a href="/feedback">表单提交</a><a href="/upload">文件上传</a><a href="/errors">异常处理</a></nav>`;
const page = (title, body) => `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title} · WebFix 测试靶场</title><style>body{margin:0;background:#f2f6fb;color:#23324a;font:15px system-ui}header,main{max-width:1000px;margin:auto;padding:20px}header{padding-bottom:0}nav{display:flex;gap:8px;flex-wrap:wrap;margin:16px 0}nav a{color:#285eaa;background:#fff;border:1px solid #dce7f5;border-radius:8px;padding:9px;text-decoration:none}main>section{background:white;border:1px solid #dce7f5;border-radius:14px;padding:24px;box-shadow:0 8px 30px #1d4b7510}input,textarea,button{font:inherit;padding:10px;border:1px solid #c6d5e8;border-radius:7px}input,textarea{display:block;box-sizing:border-box;margin:6px 0 14px;width:min(100%,500px)}button{background:#316ed4;color:white;cursor:pointer}table{border-collapse:collapse;width:100%}td,th{border:1px solid #dce7f5;padding:10px;text-align:left}.notice{background:#fff6df;padding:12px;border-radius:8px;color:#785515}code{word-break:break-all}</style></head><body><header><h1>WebFix 本地演示靶场</h1><p class="notice">故意存在安全漏洞，仅使用虚构数据；仅限本机隔离测试，切勿部署到公网或输入真实信息。重启后数据清空。</p>${nav}</header><main><section><h2>${title}</h2>${body}</section></main></body></html>`;
const escape = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const rows = (items, fields) => items.map(item => `<tr>${fields.map(field => `<td>${escape(item[field])}</td>`).join('')}</tr>`).join('');
const html = (res, title, body, status = 200) => { res.writeHead(status, {'Content-Type':'text/html; charset=utf-8','Cache-Control':'no-store','X-Content-Type-Options':'nosniff'}); res.end(page(title, body)); };
const redirect = (res, location, cookie) => { res.writeHead(303, {'Location':location,...(cookie ? {'Set-Cookie':cookie} : {})}); res.end(); };
const getBody = async req => { const chunks = []; let size = 0; for await (const chunk of req) { size += chunk.length; if (size > 1024 * 1024) throw new Error('请求超过 1 MB'); chunks.push(chunk); } return Buffer.concat(chunks); };
const bodyParams = async req => new URLSearchParams((await getBody(req)).toString('utf8'));

const server = http.createServer(async (req, res) => {
  const requestHost = req.headers.host || '';
  if (!new RegExp(`^(127\\.0\\.0\\.1|localhost):${port}$`, 'i').test(requestHost)) { res.writeHead(403); return res.end('Invalid host'); }
  const url = new URL(req.url, `http://${host}:${port}`);
  const p = url.pathname;
  try {
    if (p === '/' && req.method === 'GET') return html(res, '测试入口', `<p>本靶场模拟企业管理系统，提供登录、用户管理、商品搜索、反馈、上传和异常页面。所有数据均为虚构。</p><p>演示账号：<code>alice / demo123</code>、<code>bob / demo123</code>。请使用导航逐页测试。</p><p>用户管理、搜索、反馈、上传和异常分别覆盖不同的权限、输入、输出或业务逻辑缺陷。</p>`);
    if (p === '/login' && req.method === 'GET') return html(res, '登录', `<p>演示账号 alice / demo123</p><form method="post"><label>账号<input name="username"></label><label>密码<input type="password" name="password"></label><button>登录</button></form>`);
    if (p === '/login' && req.method === 'POST') {
      const form = await bodyParams(req);
      const user = db.prepare('SELECT * FROM users WHERE username = ?').get(form.get('username') || '');
      if (!user || user.password !== form.get('password')) return html(res, '登录失败', '<p>账号或密码错误。<a href="/login">返回登录</a></p>', 401);
      return redirect(res, '/users?id=' + user.id, `demo_user=${user.id}; Path=/; SameSite=Lax`);
    }
    if (p === '/users' && req.method === 'GET') {
      const cookie = /(?:^|;\s*)demo_user=(\d+)/.exec(req.headers.cookie || '');
      if (!cookie) return redirect(res, '/login');
      const id = Number(url.searchParams.get('id') || cookie[1]);
      const user = db.prepare('SELECT id, username, role, email FROM users WHERE id = ?').get(id);
      if (!user) return html(res, '用户管理', '<p>用户不存在</p>', 404);
      return html(res, '用户管理', `<p>当前会话用户 ID：${escape(cookie[1])}</p><p>查看指定用户：<a href="/users?id=1">用户 1</a> · <a href="/users?id=2">用户 2</a> · <a href="/users?id=3">用户 3</a></p><table><tr><th>ID</th><th>用户名</th><th>角色</th><th>邮箱</th></tr>${rows([user], ['id','username','role','email'])}</table>`);
    }
    if (p === '/products' && req.method === 'GET') {
      const q = url.searchParams.get('q') || '';
      let result;
      try { result = db.prepare(`SELECT id, name, price FROM products WHERE name LIKE '%${q}%'`).all(); }
      catch (e) { return html(res, '数据列表', `<form><label>商品搜索<input name="q" value="${escape(q)}"></label><button>搜索</button></form><p>数据库异常：${escape(e.message)}</p>`, 500); }
      return html(res, '数据列表', `<form><label>商品搜索<input name="q" value="${escape(q)}"></label><button>搜索</button></form><table><tr><th>ID</th><th>商品</th><th>价格</th></tr>${rows(result, ['id','name','price'])}</table>`);
    }
    if (p === '/feedback' && req.method === 'GET') return html(res, '反馈表单', `<form method="post"><label>姓名<input name="name"></label><label>邮箱<input name="email"></label><label>反馈内容<textarea name="message"></textarea></label><button>提交反馈</button></form><h3>最近反馈</h3>${messages.map(m => `<article><strong>${escape(m.name)}</strong>：${m.message}</article>`).join('')}`);
    if (p === '/feedback' && req.method === 'POST') {
      const form = await bodyParams(req);
      messages.push({name:form.get('name') || '',email:form.get('email') || '',message:form.get('message') || ''});
      if (messages.length > 30) messages.shift();
      return redirect(res, '/feedback');
    }
    if (p === '/upload' && req.method === 'GET') return html(res, '文件上传', `<p>文件仅保存在进程内存中，不写入磁盘或执行；重启后清空。最大请求 1 MB。</p><form method="post" enctype="multipart/form-data"><label>选择文件<input type="file" name="file"></label><button>上传</button></form><h3>已上传</h3><ul>${uploads.map(x => `<li>${escape(x.name)}（${x.size} 字节）</li>`).join('')}</ul>`);
    if (p === '/upload' && req.method === 'POST') {
      const raw = await getBody(req);
      const contentType = req.headers['content-type'] || '';
      const request = new Request('http://127.0.0.1/', {method:'POST', headers:{'content-type':contentType}, body:raw});
      const file = (await request.formData()).get('file');
      if (!(file instanceof File) || !file.name) return html(res, '文件上传', '<p>请选择文件。</p><a href="/upload">返回</a>', 400);
      uploads.push({name:file.name, size:file.size, bytes:Buffer.from(await file.arrayBuffer())});
      if (uploads.length > 10) uploads.shift();
      return redirect(res, '/upload');
    }
    if (p === '/errors' && req.method === 'GET') {
      if (url.searchParams.has('fail')) return html(res, '异常处理', `<p>服务内部异常：TypeError: Cannot read properties of undefined (reading 'total')</p><pre>at calculateTotal (C:\\demo\\billing\\service.js:42:17)\nat processOrder (C:\\demo\\routes\\orders.js:18:5)</pre>`, 500);
      return html(res, '异常处理', `<p>正常情况下显示订单总计。</p><p><a href="/errors?fail=1">模拟订单计算异常</a></p><button id="broken">模拟前端按钮异常</button><script>document.getElementById('broken').onclick=()=>{ window.demoMissing.calculate() }</script>`);
    }
    return html(res, '页面不存在', '<p>找不到该页面。</p>', 404);
  } catch (err) {
    return html(res, '请求处理失败', `<p>演示异常：${escape(err.message)}</p>`, 400);
  }
});
server.listen(port, host, () => console.log(`WebFix 本地演示靶场：http://${host}:${port}/`));
