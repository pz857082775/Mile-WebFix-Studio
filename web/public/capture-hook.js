// WebFix Studio 巡检钩子：注入到被代理页面，抓取控制台/异常/网络/资源错误与 DOM 问题
(function () {
  var script = document.currentScript;
  var q = new URLSearchParams(((script && script.src) || '').split('?')[1] || '');
  var token = q.get('token') || '';
  var target = q.get('target') || '';
  var sid = q.get('sid') || '';
  var prefix = '/proxy/' + token;

  function b64url(s) {
    return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }

  // 工具自身的路由绝不能被转发到目标站
  var TOOL_PATHS = /^\/(api\/capture|capture-hook\.js)(\?|$)/;

  function proxify(raw) {
    if (!raw || /^(data:|javascript:|mailto:|tel:|about:|#)/i.test(raw)) return raw;
    if (raw.indexOf('/proxy/') === 0 || TOOL_PATHS.test(raw)) return raw;
    try {
      var u = new URL(raw, location.href);
      if (u.protocol !== 'http:' && u.protocol !== 'https:') return raw;
      if (u.pathname.indexOf('/proxy/') === 0) return raw;
      if (u.origin === location.origin) {
        // 相对路径 / 用 location.origin 拼出的地址：属于目标站的请求，必须转发，不能打到工具自身
        if (TOOL_PATHS.test(u.pathname)) return raw;
        return prefix + u.pathname + u.search + u.hash;
      }
      return '/proxy/' + b64url(u.origin) + u.pathname + u.search + u.hash;
    } catch (e) { return raw; }
  }

  function b64urlDecode(s) {
    s = s.replace(/-/g, '+').replace(/_/g, '/');
    while (s.length % 4) s += '=';
    return atob(s);
  }

  function unproxy(u) {
    try {
      var x = new URL(u, location.href);
      if (x.origin === location.origin) {
        var m = x.pathname.match(/^\/proxy\/([^/]+)(\/.*)?$/);
        if (m) return b64urlDecode(m[1]) + (m[2] || '/') + x.search;
      }
    } catch (e) { /* 忽略 */ }
    return u;
  }

  function fmt(a) {
    try {
      if (typeof a === 'string') return a;
      if (a instanceof Error) return a.stack || a.message;
      return JSON.stringify(a);
    } catch (e) { return String(a); }
  }

  function send(type, data) {
    if (!sid) return;
    try {
      var payload = Object.assign({ sid: sid, token: token, type: type, pageUrl: location.href, ts: Date.now() }, data);
      fetch('/api/capture', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      }).catch(function () {});
    } catch (e) { /* 忽略 */ }
  }

  // ---- 控制台错误 ----
  ['error', 'warn'].forEach(function (level) {
    var orig = console[level].bind(console);
    console[level] = function () {
      var args = [].slice.call(arguments);
      send('console', { level: level, message: args.map(fmt).join(' ').slice(0, 2000) });
      orig.apply(null, args);
    };
  });

  // ---- JS 异常与资源加载失败 ----
  window.addEventListener('error', function (e) {
    var t = e.target;
    if (t && t !== window && (t.src || t.href)) {
      send('resource', { message: '资源加载失败: ' + unproxy(t.src || t.href), tag: t.tagName });
    } else {
      send('exception', {
        message: e.message || '未知异常',
        stack: ((e.error && e.error.stack) || '').slice(0, 2000),
        source: (e.filename || '') + ':' + (e.lineno || 0)
      });
    }
  }, true);

  window.addEventListener('unhandledrejection', function (e) {
    send('exception', { message: '未处理的 Promise 拒绝: ' + fmt(e.reason).slice(0, 1000) });
  });

  // ---- fetch / XHR：重写目标站绝对地址 + 记录失败 ----
  function isOurs(url) {
    return String(url).indexOf('/api/capture') === 0 || String(url).indexOf('/capture-hook.js') === 0;
  }

  var origFetch = window.fetch;
  window.fetch = function (input, init) {
    var url = typeof input === 'string' ? input : (input && input.url) || '';
    var rewritten = isOurs(url) ? url : proxify(url);
    var arg = input;
    if (rewritten !== url) {
      arg = (typeof input === 'string') ? rewritten : new Request(rewritten, input);
    }
    var p = origFetch.call(this, arg, init);
    if (!isOurs(url)) {
      p.then(function (r) {
        if (!r.ok) send('network', { message: 'HTTP ' + r.status + ' ' + r.statusText, url: rewritten, status: r.status });
      }).catch(function (err) {
        send('network', { message: '请求失败: ' + err.message, url: rewritten });
      });
    }
    return p;
  };

  var origOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url) {
    var rest = [].slice.call(arguments, 2);
    this.__wfUrl = isOurs(url) ? String(url) : proxify(String(url));
    var xhr = this;
    this.addEventListener('loadend', function () {
      if (!isOurs(xhr.__wfUrl) && (xhr.status === 0 || xhr.status >= 400)) {
        send('network', { message: 'HTTP ' + (xhr.status || '网络错误') + ' ' + method + ' ' + unproxy(xhr.__wfUrl), url: xhr.__wfUrl, status: xhr.status });
      }
    });
    return origOpen.apply(this, [method, this.__wfUrl].concat(rest));
  };

  // ---- window.open：新窗口也走代理，避免逃出巡检环境 ----
  var origWindowOpen = window.open;
  window.open = function (url) {
    var rest = [].slice.call(arguments, 1);
    return origWindowOpen.apply(this, [url ? proxify(String(url)) : url].concat(rest));
  };

  // ---- SPA 路由与动态 DOM 链接重写 ----
  ['pushState', 'replaceState'].forEach(function (fn) {
    var orig = history[fn].bind(history);
    history[fn] = function (s, t, url) {
      return orig(s, t, url ? proxify(String(url)) : url);
    };
  });

  function rewriteEl(el) {
    if (!el || el.nodeType !== 1 || !el.getAttribute) return;
    ['href', 'src', 'action'].forEach(function (attr) {
      var v = el.getAttribute(attr);
      if (v) {
        var nv = proxify(v);
        if (nv !== v) el.setAttribute(attr, nv);
      }
    });
  }

  function rewriteAll(root) {
    rewriteEl(root);
    if (root.querySelectorAll) {
      root.querySelectorAll('a[href],img[src],script[src],link[href],form[action]').forEach(rewriteEl);
    }
  }

  // ---- DOM 体检（UI 问题，覆盖 SPA 动态渲染内容） ----
  var domTimer;
  function domCheck() {
    var issues = [];
    var ids = [].map.call(document.querySelectorAll('[id]'), function (e) { return e.id; });
    var dup = ids.filter(function (v, i) { return ids.indexOf(v) !== i; });
    if (dup.length) issues.push({ ruleId: 'ui-duplicate-id', evidence: '重复 id: ' + dup.filter(function (v, i, a) { return a.indexOf(v) === i; }).join(', ') });
    var noAlt = [].filter.call(document.images, function (i) { return !i.hasAttribute('alt'); });
    if (noAlt.length) issues.push({ ruleId: 'ui-img-no-alt', evidence: noAlt.length + ' 个图片缺少 alt' });
    if (!document.querySelector('meta[name="viewport"]')) issues.push({ ruleId: 'ui-missing-viewport', evidence: '缺少 viewport 元标签' });
    var unlabeled = [].filter.call(document.querySelectorAll('input:not([type=hidden]):not([type=submit]):not([type=button])'), function (i) {
      if (i.getAttribute('aria-label')) return false;
      if (i.id && document.querySelector('label[for="' + i.id + '"]')) return false;
      if (i.closest('label')) return false;
      return true;
    });
    if (unlabeled.length) issues.push({ ruleId: 'ui-input-no-label', evidence: unlabeled.length + ' 个输入框缺少 label 关联' });
    if (issues.length) send('dom', { issues: issues });
  }
  function scheduleCheck() {
    clearTimeout(domTimer);
    domTimer = setTimeout(domCheck, 1500);
  }

  var mo = new MutationObserver(function (muts) {
    for (var i = 0; i < muts.length; i++) {
      var m = muts[i];
      if (m.type === 'childList') {
        m.addedNodes.forEach(rewriteAll);
      } else if (m.type === 'attributes') {
        rewriteEl(m.target);
      }
    }
    scheduleCheck();
  });

  function start() {
    rewriteAll(document.documentElement);
    mo.observe(document.documentElement, {
      childList: true, subtree: true, attributes: true,
      attributeFilter: ['href', 'src', 'action']
    });
    send('page', { title: document.title });
    scheduleCheck();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
})();