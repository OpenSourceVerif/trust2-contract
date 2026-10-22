/* Shared, dependency-free data and anchoring rules; also exercised by Node tests. */
(function (root) {
  'use strict';
  const authorPattern = /^[a-z0-9](?:[a-z0-9-]{0,37}[a-z0-9])?$/;
  function validate(bundle) {
    if (!bundle || bundle.version !== 1 || bundle.document !== 'pcg-paper' ||
        typeof bundle.author !== 'string' || !authorPattern.test(bundle.author) || !Array.isArray(bundle.annotations) || bundle.annotations.length > 10000)
      throw new Error('批注文件格式不正确，或作者标识不是小写 GitHub 用户名。');
    const ids = new Set();
    for (const a of bundle.annotations) {
      if (!a || typeof a.id !== 'string' || !/^[a-zA-Z0-9_-]{1,100}$/.test(a.id) || ids.has(a.id) ||
          typeof a.updatedAt !== 'string' || !/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z$/.test(a.updatedAt) || !Number.isFinite(Date.parse(a.updatedAt)) || new Date(a.updatedAt).toISOString() !== a.updatedAt ||
          typeof a.deleted !== 'boolean' || typeof a.note !== 'string' || a.note.length > 20000 ||
          !a.anchor || typeof a.anchor.section !== 'string' || a.anchor.section.length > 100 ||
          typeof a.anchor.exact !== 'string' || !a.anchor.exact.trim() || a.anchor.exact.length > 10000 ||
          typeof a.anchor.prefix !== 'string' || a.anchor.prefix.length > 64 ||
          typeof a.anchor.suffix !== 'string' || a.anchor.suffix.length > 64 ||
          !Number.isSafeInteger(a.anchor.start) || a.anchor.start < 0)
        throw new Error('批注条目无效或 ID 重复，未导入任何内容。');
      ids.add(a.id);
    }
    return bundle;
  }
  const key = a => a.author + ':' + a.id;
  // Newer revisions win, including tombstones. Equal revisions prefer the later source.
  function merge(...sources) {
    const map = new Map();
    for (const source of sources) for (const a of source) {
      const prev = map.get(key(a));
      if (!prev || a.updatedAt >= prev.updatedAt) map.set(key(a), a);
    }
    return [...map.values()].sort((a, b) => key(a).localeCompare(key(b)));
  }
  function flatten(bundles) {
    return bundles.flatMap(b => validate(b).annotations.map(a => ({...a, author: b.author})));
  }
  function bundle(author, rows) {
    return {version: 1, document: 'pcg-paper', author, annotations: rows.filter(a => a.author === author).map(({author: _, ...a}) => a)};
  }
  function locate(text, a) {
    const candidates = [];
    for (let pos = text.indexOf(a.exact); pos !== -1; pos = text.indexOf(a.exact, pos + 1)) {
      const prefix = a.prefix && text.slice(Math.max(0, pos - a.prefix.length), pos) === a.prefix;
      const suffix = a.suffix && text.slice(pos + a.exact.length, pos + a.exact.length + a.suffix.length) === a.suffix;
      candidates.push({start: pos, end: pos + a.exact.length, score: Number(!!prefix) + Number(!!suffix)});
    }
    if (candidates.length === 1) return candidates[0];
    // Never guess at repeated text after an edit. Context must identify one candidate.
    candidates.sort((a, b) => b.score - a.score);
    return candidates[0]?.score > (candidates[1]?.score || 0) ? candidates[0] : null;
  }
  const api = {validate, merge, flatten, bundle, locate, key, authorPattern};
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else root.PaperAnnotationCore = api;
})(typeof window === 'undefined' ? globalThis : window);
