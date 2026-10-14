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
