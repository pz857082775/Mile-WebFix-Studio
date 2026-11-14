// WebFix Studio AI 自动巡检：感知 → 识别(特效) → 决策 → 操作，循环遍历内嵌页面
(function () {
  'use strict';

  var SAFE_SKIP = /删除|退出|注销|支付|付款|下单|提交订单|清空|解绑|充值|转账|停用|禁用|logout|sign\s*out|delete|remove|pay\b|checkout|purchase|unsubscribe|deactivate/i;
  var FILL_TYPES = { text: '测试数据', email: 'tester@example.com', search: '测试', tel: '13800000000', url: 'https://example.com', number: '1', password: 'Demo@12345' };

  function sleep(ms) { return new Promise(function (r) { setTimeout(r, ms); }); }

  function b64urlDecode(s) {
    s = s.replace(/-/g, '+').replace(/_/g, '/');
    while (s.length % 4) s += '=';
    return atob(s);
  }

  // 代理地址 → 真实目标地址（用于证据展示）
  function realOf(u, base) {
    try {
      var x = new URL(u, base);
      var m = x.pathname.match(/^\/proxy\/([^/]+)(\/.*)?$/);
      if (m) return b64urlDecode(m[1]) + (m[2] || '/') + x.search;
      return x.href;
    } catch (e) { return String(u); }
  }

  function parseColor(s) {
    var m = String(s).match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    var p = m[1].split(',').map(parseFloat);
    return { rgb: p.slice(0, 3), a: p.length > 3 ? p[3] : 1 };
  }
  function lum(rgb) {
    var c = rgb.map(function (v) { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); });
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
  }
  function effBg(el) {
    var n = el;
    while (n && n.nodeType === 1) {
      var c = parseColor(getComputedStyle(n).backgroundColor);
      if (c && c.a > 0.05) return c.rgb;
      n = n.parentElement;
    }
    return [255, 255, 255];
  }
  function contrast(el) {
    var fg = parseColor(getComputedStyle(el).color);
    if (!fg || fg.a < 0.5) return null;
    var bg = effBg(el);
    var l1 = lum(fg.rgb), l2 = lum(bg);
    var ratio = (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
    var cs = getComputedStyle(el);
    var size = parseFloat(cs.fontSize) || 16;
    var bold = (parseInt(cs.fontWeight, 10) || 400) >= 700;
    var need = (size >= 24 || (size >= 18.66 && bold)) ? 3 : 4.5;
    return { ratio: ratio, need: need, fg: fg.rgb, bg: bg };
  }

  function labelOf(el) {
    var t = (el.getAttribute('aria-label') || el.textContent || el.getAttribute('title') ||
      el.getAttribute('placeholder') || el.getAttribute('name') || el.getAttribute('alt') || '').replace(/\s+/g, ' ').trim();
    return t.slice(0, 30);
  }

  function visible(el) {
    var r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) return false;
    var cs = getComputedStyle(el);
    return cs.visibility !== 'hidden' && cs.display !== 'none';
  }

  var FX_CSS = [
    '#wf-overlay{position:fixed;inset:0;pointer-events:none;z-index:2147483646;font-family:system-ui,sans-serif;overflow:hidden}',
    '.wf-box{position:absolute;border:1.5px dashed rgba(56,189,248,.9);border-radius:4px;box-shadow:inset 0 0 12px rgba(56,189,248,.18);transition:all .2s}',
    '.wf-tag{position:absolute;top:-18px;left:-1px;font-size:10px;line-height:15px;padding:0 5px;background:rgba(2,132,199,.92);color:#fff;border-radius:3px 3px 3px 0;white-space:nowrap;max-width:220px;overflow:hidden;text-overflow:ellipsis}',
    '.wf-box.issue{border-color:rgba(248,113,113,.95);animation:wf-pulse 1s ease-in-out infinite}',
    '.wf-box.issue .wf-tag{background:rgba(220,38,38,.92)}',
    '.wf-box.active{border-style:solid;border-color:#22d3ee;box-shadow:0 0 16px rgba(34,211,238,.65)}',
    '.wf-scan{position:absolute;left:0;right:0;top:-12%;height:90px;background:linear-gradient(180deg,transparent,rgba(56,189,248,.15) 45%,rgba(56,189,248,.5) 50%,rgba(56,189,248,.15) 55%,transparent);animation:wf-scan 1.6s linear forwards}',
    '@keyframes wf-scan{to{transform:translateY(135vh)}}',
    '.wf-ripple{position:absolute;width:14px;height:14px;margin:-7px 0 0 -7px;border-radius:50%;border:2px solid #22d3ee;animation:wf-ripple .7s ease-out forwards}',
    '@keyframes wf-ripple{from{transform:scale(.4);opacity:1}to{transform:scale(4.5);opacity:0}}',
    '@keyframes wf-pulse{0%,100%{box-shadow:0 0 0 0 rgba(248,113,113,.45)}50%{box-shadow:0 0 0 6px rgba(248,113,113,0)}}',
    '.wf-hud{position:fixed;top:10px;right:10px;display:flex;align-items:center;gap:6px;padding:5px 12px;border-radius:999px;background:rgba(2,6,23,.82);border:1px solid rgba(56,189,248,.5);color:#7dd3fc;font-size:12px;backdrop-filter:blur(4px)}',
    '.wf-hud .dot{width:8px;height:8px;border-radius:50%;background:#22d3ee;animation:wf-blink 1s ease-in-out infinite}',
    '@keyframes wf-blink{0%,100%{opacity:1;box-shadow:0 0 8px #22d3ee}50%{opacity:.35;box-shadow:none}}'
  ].join('\n');

  function Pilot(frame, opts) {
    this.frame = frame;
    this.opts = Object.assign({ fx: true, allowSubmit: false, maxSteps: 24, onLog: function () {}, onIssues: function () {}, getUrl: function () { return ''; } }, opts || {});
    this.running = false;
    this.step = 0;
    this.visited = new Set();
    this.filled = new Set();
    this._lastDoc = null;
  }

  Pilot.prototype.log = function (msg, cls) { this.opts.onLog(msg, cls || ''); };

  Pilot.prototype.stop = function () {
    this.running = false;
    try {
      var doc = this.frame.contentDocument;
      var ov = doc && doc.getElementById('wf-overlay');
      if (ov) ov.remove();
      var st = doc && doc.getElementById('wf-fx-style');
      if (st) st.remove();
    } catch (e) { /* 忽略 */ }
  };

  Pilot.prototype.ensureOverlay = function (doc) {
    if (!this.opts.fx) return null;
    var ov = doc.getElementById('wf-overlay');
    if (!ov) {
      var style = doc.createElement('style');
      style.id = 'wf-fx-style';
      style.textContent = FX_CSS;
      doc.head.appendChild(style);
      ov = doc.createElement('div');
      ov.id = 'wf-overlay';
      ov.innerHTML = '<div class="wf-hud"><span class="dot"></span><span class="wf-hud-text">AI 识别中</span></div>';
      doc.body.appendChild(ov);
      var scan = doc.createElement('div');
      scan.className = 'wf-scan';
      ov.appendChild(scan);
      setTimeout(function () { scan.remove(); }, 1800);
    }
    return ov;
  };

  Pilot.prototype.hud = function (doc, text) {
    if (!this.opts.fx) return;
    var ov = doc.getElementById('wf-overlay');
    var t = ov && ov.querySelector('.wf-hud-text');
    if (t) t.textContent = text;
  };

  // ---- 感知：识别可操作元素 + 检出 UI 问题 ----
  Pilot.prototype.perceive = function (doc) {
    var actions = [];
    var issueEls = { tap: [], contrast: [], empty: [], alt: [], broken: [] };
    var base = doc.location.href;
    var pageKey = doc.location.pathname + doc.location.search;
    var self = this;

    function addAction(el, kind, key, extra) {
      actions.push(Object.assign({ el: el, kind: kind, key: key, label: labelOf(el) }, extra || {}));
    }

    // 链接
    doc.querySelectorAll('a[href]').forEach(function (a, i) {
      if (!visible(a)) return;
      var raw = a.getAttribute('href') || '';
      if (/^(javascript:|mailto:|tel:|data:)/i.test(raw)) return;
      if (raw === '#' || raw === '') return;
      if (a.target === '_blank' || a.hasAttribute('download')) return;
      var href = a.href;
      if (SAFE_SKIP.test(labelOf(a) + ' ' + raw)) return;
      addAction(a, 'link', href, { href: href });
      var r = a.getBoundingClientRect();
      if (r.width < 40 || r.height < 40) issueEls.tap.push({ el: a, text: '「' + (labelOf(a) || '链接' + i) + '」' + Math.round(r.width) + '×' + Math.round(r.height) + 'px' });
      if (!labelOf(a) && !a.querySelector('img[alt]:not([alt=""])') && !a.querySelector('svg title')) issueEls.empty.push({ el: a, text: '<a href="' + raw.slice(0, 60) + '"> 无文字/无标签' });
    });

    // 按钮
    doc.querySelectorAll('button, [role="button"], input[type=submit], input[type=button]').forEach(function (b, i) {
      if (!visible(b)) return;
      var label = labelOf(b) || (b.value || '');
      if (SAFE_SKIP.test(label)) return;
      var isSubmit = b.type === 'submit' || (b.tagName === 'BUTTON' && !b.type && b.closest('form'));
      var form = b.closest('form');
      var isLogin = form && !!form.querySelector('input[type=password]');
      if (isSubmit && !self.opts.allowSubmit && !isLogin) return;
      addAction(b, isLogin && isSubmit ? 'login' : 'button', pageKey + '|btn|' + i + '|' + label, { form: form, isLogin: isLogin });
      var r = b.getBoundingClientRect();
      if (r.width < 40 || r.height < 40) issueEls.tap.push({ el: b, text: '「' + (label || '按钮' + i) + '」' + Math.round(r.width) + '×' + Math.round(r.height) + 'px' });
      if (!label && !b.querySelector('img[alt]:not([alt=""])') && !b.querySelector('svg title')) issueEls.empty.push({ el: b, text: '<button> 无文字/无标签' });
    });

    // 输入框 / 下拉 / 勾选
    doc.querySelectorAll('input, textarea, select').forEach(function (el, i) {
      if (!visible(el)) return;
      var tag = el.tagName.toLowerCase();
      var type = (el.type || 'text').toLowerCase();
      var key = pageKey + '|in|' + (el.name || el.id || i);
      if (tag === 'select') { addAction(el, 'select', key); return; }
      if (tag === 'textarea' || FILL_TYPES[type]) { addAction(el, 'fill', key, { fillType: type }); return; }
      if (type === 'checkbox' || type === 'radio') addAction(el, 'toggle', key);
    });

    // 图片问题
    doc.querySelectorAll('img').forEach(function (img) {
      if (!img.hasAttribute('alt')) issueEls.alt.push({ el: img, text: realOf(img.src || img.getAttribute('src') || '', base).slice(0, 100) });
      if (img.src && img.complete && img.naturalWidth === 0 && !/^data:/i.test(img.src)) {
        issueEls.broken.push({ el: img, text: realOf(img.src, base).slice(0, 120) });
      }
    });

    // 文本对比度（限量，避免大页面卡顿）
    var checked = 0;
    var nodes = doc.querySelectorAll('a,button,h1,h2,h3,p,label,li,span,td,th');
    for (var i = 0; i < nodes.length && checked < 120; i++) {
      var el = nodes[i];
      var own = '';
      for (var j = 0; j < el.childNodes.length; j++) {
        if (el.childNodes[j].nodeType === 3) own += el.childNodes[j].textContent;
      }
      own = own.trim();
      if (!own || !visible(el)) continue;
      checked++;
      var c = contrast(el);
      if (c && c.ratio < c.need) {
        issueEls.contrast.push({ el: el, text: '「' + own.slice(0, 20) + '」' + c.ratio.toFixed(1) + ':1（要求≥' + c.need + ':1）' });
      }
    }

    // 汇总问题（每类一条证据，最多列 3 个示例）
    var issues = [];
    function report(ruleId, list, prefix) {
      if (!list.length) return;
      var sample = list.slice(0, 3).map(function (x) { return x.text; }).join('；');
      issues.push({ ruleId: ruleId, evidence: prefix + '（' + list.length + ' 处）：' + sample, els: list.map(function (x) { return x.el; }) });
    }
    report('ui-small-tap-target', issueEls.tap, '点击目标过小');
    report('ui-low-contrast', issueEls.contrast, '文本对比度不足');
    report('ui-empty-clickable', issueEls.empty, '空按钮/空链接');
    report('ui-img-no-alt', issueEls.alt, '图片缺少 alt');
    report('func-resource-error', issueEls.broken, '图片加载失败');

    return { actions: actions, issues: issues };
  };

  // ---- 决策：选下一个未操作过的安全动作 ----
  Pilot.prototype.decide = function (doc, actions) {
    var self = this;
    function fresh(a) {
      if (a.kind === 'fill') return !self.filled.has(a.key);
      return !self.visited.has(a.key);
    }
    // 1) 先填输入框
    var fill = actions.find(function (a) { return a.kind === 'fill' && fresh(a); });
    if (fill) return fill;
    // 2) 登录表单：输入填完后自动提交
    var login = actions.find(function (a) {
      if (a.kind !== 'login' || self.visited.has(a.key)) return false;
      var inputs = a.form.querySelectorAll('input[type=password],input[type=text],input[type=email]');
      return [].every.call(inputs, function (inp) {
        return inp.type === 'password' ? inp.value.length > 0 : true;
      });
    });
    if (login) return login;
    // 3) 未访问链接
    var link = actions.find(function (a) { return a.kind === 'link' && fresh(a); });
    if (link) return link;
    // 4) 普通按钮 / 下拉 / 勾选
    return actions.find(function (a) { return ['button', 'select', 'toggle'].indexOf(a.kind) >= 0 && fresh(a); }) || null;
  };

  // ---- 特效绘制 ----
  Pilot.prototype.drawBoxes = function (doc, actions, issues) {
    if (!this.opts.fx) return;
    var ov = this.ensureOverlay(doc);
    if (!ov) return;
    ov.querySelectorAll('.wf-box').forEach(function (b) { b.remove(); });
    var issueSet = new Set();
    issues.forEach(function (it) { (it.els || []).forEach(function (el) { issueSet.add(el); }); });
    var boxed = new Set();
    actions.slice(0, 40).forEach(function (a) {
      var r = a.el.getBoundingClientRect();
      if (r.width <= 0 || r.height <= 0) return;
      boxed.add(a.el);
      var box = doc.createElement('div');
      box.className = 'wf-box' + (issueSet.has(a.el) ? ' issue' : '');
      box.style.left = r.left + 'px';
      box.style.top = r.top + 'px';
      box.style.width = r.width + 'px';
      box.style.height = r.height + 'px';
      var kindLabel = { link: '链接', button: '按钮', login: '登录', fill: '输入', select: '下拉', toggle: '勾选' }[a.kind] || a.kind;
      var tag = doc.createElement('span');
      tag.className = 'wf-tag';
      tag.textContent = kindLabel + (a.label ? ' · ' + a.label : '');
      box.appendChild(tag);
      ov.appendChild(box);
      a._box = box;
    });
    // 不在候选里的问题元素（如图片）也标红
    issueSet.forEach(function (el) {
      if (!el.isConnected || boxed.has(el)) return;
      var r = el.getBoundingClientRect();
      if (r.width <= 0 || r.height <= 0) return;
      var box = doc.createElement('div');
      box.className = 'wf-box issue';
      box.style.left = r.left + 'px';
      box.style.top = r.top + 'px';
      box.style.width = r.width + 'px';
      box.style.height = r.height + 'px';
      var tag = doc.createElement('span');
      tag.className = 'wf-tag';
      tag.textContent = '问题';
      box.appendChild(tag);
      ov.appendChild(box);
    });
  };

  Pilot.prototype.ripple = function (doc, el) {
    if (!this.opts.fx) return;
    var ov = doc.getElementById('wf-overlay');
    if (!ov) return;
    var r = el.getBoundingClientRect();
    var rp = doc.createElement('div');
    rp.className = 'wf-ripple';
    rp.style.left = (r.left + r.width / 2) + 'px';
    rp.style.top = (r.top + r.height / 2) + 'px';
    ov.appendChild(rp);
    setTimeout(function () { rp.remove(); }, 800);
  };

  function setInputValue(el, value) {
    var win = el.ownerDocument.defaultView;
    var proto = el.tagName === 'TEXTAREA' ? win.HTMLTextAreaElement.prototype : win.HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, value);
    el.dispatchEvent(new win.Event('input', { bubbles: true }));
    el.dispatchEvent(new win.Event('change', { bubbles: true }));
  }

  // ---- 执行动作 ----
  Pilot.prototype.act = async function (doc, a) {
    var el = a.el;
    el.scrollIntoView({ block: 'center' });
    await sleep(200);
    if (a._box) a._box.classList.add('active');
    this.ripple(doc, el);
    await sleep(420);
    var kindLabel = { link: '点击链接', button: '点击按钮', login: '提交登录', fill: '填写输入', select: '选择下拉', toggle: '切换勾选' }[a.kind] || '操作';
    this.log('第 ' + this.step + ' 步：' + kindLabel + '「' + (a.label || el.name || el.tagName) + '」', 'act');

    if (a.kind === 'fill') {
      el.focus();
      setInputValue(el, FILL_TYPES[a.fillType] || '测试数据');
      this.filled.add(a.key);
    } else if (a.kind === 'select') {
      if (el.options.length > 1) {
        el.selectedIndex = 1;
        el.dispatchEvent(new doc.defaultView.Event('change', { bubbles: true }));
      }
      this.visited.add(a.key);
    } else {
      this.visited.add(a.key);
      el.click();
    }
  };

  // ---- 主循环 ----
  Pilot.prototype.run = async function () {
    if (this.running) return;
    this.running = true;
    this.step = 0;
    this.visited.clear();
    this.filled.clear();
    this._lastDoc = null;
    this.log('AI 自动巡检启动：识别界面元素 → 安全操作 → 抓取问题（危险按钮自动跳过）', 'ok');
    var started = Date.now();
    try {
      while (this.running && this.step < this.opts.maxSteps && Date.now() - started < 150000) {
        var doc = null;
        try { doc = this.frame.contentDocument; } catch (e) { /* 跨域瞬间 */ }
        if (!doc || !doc.body || doc.readyState === 'loading') { await sleep(500); continue; }

        if (doc !== this._lastDoc) {
          this._lastDoc = doc;
          this.ensureOverlay(doc);
          this.log('识别页面：' + (this.opts.getUrl() || doc.location.href), 'ok');
        }

        var step = ++this.step;
        this.hud(doc, 'AI 识别中 · 第 ' + step + ' 步');
        var result = this.perceive(doc);
        this.log('识别到 ' + result.actions.length + ' 个可操作元素' + (result.issues.length ? '，发现 ' + result.issues.length + ' 类界面问题' : ''), '');
        if (result.issues.length) this.opts.onIssues(result.issues, this.opts.getUrl() || doc.location.href);
        this.drawBoxes(doc, result.actions, result.issues);
        await sleep(this.opts.fx ? 900 : 150);
        if (!this.running) break;

        var target = this.decide(doc, result.actions);
        if (!target) {
          this.log('本页可操作元素已全部遍历，AI 巡检完成', 'ok');
          break;
        }
        await this.act(doc, target);
        await sleep(1300);
      }
      if (this.step >= this.opts.maxSteps) this.log('已达最大步数（' + this.opts.maxSteps + '），巡检结束', 'warn');
    } catch (e) {
      this.log('AI 巡检异常：' + e.message, 'warn');
    } finally {
      this.stop();
      this.running = false;
      this.log('AI 巡检结束：共执行 ' + this.step + ' 步', 'ok');
      if (this.opts.onDone) this.opts.onDone();
    }
  };

  window.WfPilot = Pilot;
})();