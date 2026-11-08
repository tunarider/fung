// fung manual: a tiny highlighter for fung code blocks and the filter on
// the builtins page. No dependencies.
(function () {
  "use strict";
  var KW = /^(fn|if|else|then|match|when|for|in|not|and|or|return|import|as|type|test|assert|true|false|nil)$/;
  var RULES = [
    ["com", /^#[^\n]*/],
    ["str", /^"(?:\\.|\$\{[^}]*\}|[^"\\])*"/],
    ["str", /^`[^`]*`/],
    ["sym", /^'[A-Za-z_][A-Za-z0-9_]*/],
    ["num", /^(?:\d[\d_]*(?:\.\d+)?(?:ms|s|m|h))+(?![A-Za-z_])/],
    ["num", /^(?:0x[0-9a-fA-F_]+|0o[0-7_]+|0b[01_]+|\d[\d_]*(?:\.\d+)?(?:e[+-]?\d+)?)/],
    ["blk", /^;;/],
    ["word", /^[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\u00c0-\uffff]*/],
    ["op", /^(?:=>|\|>|\?\?|->|\.\.\.|\.\.=|\.\.)/],
    ["ch", /^[\s\S]/]
  ];
  function esc(s) { return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;"); }
  function fung(src) {
    var out = "", rest = src;
    while (rest.length) {
      for (var i = 0; i < RULES.length; i++) {
        var m = RULES[i][1].exec(rest);
        if (!m) continue;
        var t = m[0], kind = RULES[i][0];
        if (kind === "word") kind = KW.test(t) ? "kw" : (/^[A-Z]/.test(t) ? "tag" : "");
        if (kind === "op" || kind === "ch") kind = "";
        out += kind ? '<span class="tk-' + kind + '">' + esc(t) + "</span>" : esc(t);
        rest = rest.slice(t.length);
        break;
      }
    }
    return out;
  }
  function core(src) {
    // core trees: colour the head of every (form ...)
    return esc(src).replace(/\((\w+)/g, '(<span class="tk-head">$1</span>');
  }
  document.querySelectorAll("pre > code").forEach(function (el) {
    var pre = el.parentElement;
    if (pre.classList.contains("plain")) return;
    el.innerHTML = pre.classList.contains("core") ? core(el.textContent) : fung(el.textContent);
  });

  var input = document.getElementById("fn-search");
  if (input) {
    var count = document.getElementById("fn-count");
    var fns = Array.prototype.slice.call(document.querySelectorAll(".fn"));
    var groups = Array.prototype.slice.call(document.querySelectorAll(".group"));
    var apply = function () {
      var q = input.value.trim().toLowerCase(), shown = 0;
      fns.forEach(function (f) {
        var hit = !q || f.textContent.toLowerCase().indexOf(q) >= 0;
        f.hidden = !hit;
        if (hit) shown++;
      });
      groups.forEach(function (g) { g.hidden = !g.querySelector(".fn:not([hidden])"); });
      count.textContent = q ? shown + "개가 맞습니다" : fns.length + "개 함수";
    };
    input.addEventListener("input", apply);
    apply();
  }
})();

// the contents drawer on small screens
(function () {
  var open = document.querySelector(".mbar-open");
  var side = document.getElementById("side");
  var scrim = document.querySelector(".scrim");
  var close = document.querySelector(".side-close");
  if (!open || !side) return;
  var mq = window.matchMedia("(max-width: 860px)");
  function set(on) {
    document.documentElement.classList.toggle("drawer", on);
    side.classList.toggle("open", on);
    scrim.hidden = !on;
    open.setAttribute("aria-expanded", on ? "true" : "false");
    if (on) {
      var cur = side.querySelector(".toc2 a.on") || side.querySelector('[aria-current="page"]');
      if (cur) cur.scrollIntoView({ block: "center" });
      close.focus();
    } else {
      open.focus({ preventScroll: true });
    }
  }
  open.addEventListener("click", function () { set(true); });
  close.addEventListener("click", function () { set(false); });
  scrim.addEventListener("click", function () { set(false); });
  document.addEventListener("keydown", function (e) { if (e.key === "Escape" && side.classList.contains("open")) set(false); });
  // following a link inside the drawer closes it (same-page anchors included)
  side.addEventListener("click", function (e) {
    var a = e.target.closest("a");
    if (!a || !mq.matches || !side.classList.contains("open")) return;
    set(false);
    // a same-page anchor: scroll after the drawer has let go of the page
    var h = a.getAttribute("href");
    if (h && h.charAt(0) === "#" && h.length > 1) {
      var t = document.getElementById(h.slice(1));
      if (t) {
        e.preventDefault();
        history.pushState(null, "", h);
        t.scrollIntoView({ block: "start" });
      }
    }
  });
  mq.addEventListener("change", function () { if (!mq.matches && side.classList.contains("open")) set(false); });
})();

// fill the version picker from ../versions.js
(function () {
  var sel = document.getElementById("ver");
  var vs = window.FUNG_VERSIONS;
  if (!sel || !vs) return;
  var cur = sel.dataset.current;
  sel.innerHTML = "";
  vs.forEach(function (v) {
    var o = document.createElement("option");
    o.value = v.dir; o.textContent = v.tag ? v.name + " (" + v.tag + ")" : v.name;
    if (v.dir === cur) { o.selected = true; document.getElementById("ver-name").textContent = v.name; var t = document.getElementById("ver-tag"); t.textContent = v.tag || ""; t.hidden = !v.tag; }
    sel.appendChild(o);
  });
  sel.addEventListener("change", function () {
    var parts = location.pathname.split("/");
    var i = parts.indexOf(cur);
    if (i >= 0) { parts[i] = sel.value; location.pathname = parts.join("/"); }
  });
})();

// highlight the section being read in the sidebar
(function () {
  var links = Array.prototype.slice.call(document.querySelectorAll(".toc2 a"));
  if (!links.length) return;
  var heads = links.map(function (a) { return document.getElementById(a.getAttribute("href").slice(1)); });
  var side = document.querySelector(".side");
  var last = null, queued = false;
  function update() {
    queued = false;
    var line = window.innerHeight * 0.25, at = 0;
    heads.forEach(function (h, i) { if (h && h.getBoundingClientRect().top <= line) at = i; });
    // at the very bottom, the last section wins even if its heading never reached the line
    if (window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2) at = heads.length - 1;
    if (links[at] === last) return;
    if (last) last.classList.remove("on");
    last = links[at];
    last.classList.add("on");
    var sec = document.querySelector(".mbar-sec");
    if (sec) sec.textContent = last.textContent;
    var r = last.getBoundingClientRect(), sr = side.getBoundingClientRect();
    if (r.top < sr.top || r.bottom > sr.bottom) last.scrollIntoView({ block: "nearest" });
  }
  window.addEventListener("scroll", function () { if (!queued) { queued = true; requestAnimationFrame(update); } }, { passive: true });
  window.addEventListener("resize", update);
  update();
})();

// An old deep link (`reference/functions.html#fn-map`, from when every
// function had its entry on that page) goes on to where the function is
// now: the page still links to it by the same anchor.
(function () {
  "use strict";
  var h = location.hash;
  if (!/^#fn-/.test(h) || document.getElementById(decodeURIComponent(h.slice(1)))) return;
  var links = document.querySelectorAll("a[href]");
  for (var i = 0; i < links.length; i++) {
    var href = links[i].getAttribute("href");
    if (href.slice(-h.length) === h && href !== h) {
      location.replace(links[i].href);
      return;
    }
  }
})();
