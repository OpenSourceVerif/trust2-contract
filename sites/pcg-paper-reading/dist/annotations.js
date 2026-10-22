(() => {
  'use strict';
  const C = window.PaperAnnotationCore;
  const $ = id => document.getElementById(id);
  const article = document.querySelector('.paper-document');
  const panel = $('annotation-panel'), list = $('annotation-list'), editor = $('annotation-editor');
  const toolbar = $('annotation-selection'), toggle = $('annotation-toggle');
  const KEY = 'trust2-contract.annotations.pcg-paper.v1';
  const PROFILE = 'trust2-contract.annotation-author.v1';
  const DRAFT = KEY + '.draft';
  const scopes = new Map([['__header', article.querySelector('header')], ...[...article.querySelectorAll('.chapter')].map(el => [el.id, el])]);
  let shared = [], local = [], author = '', filter = 'all', selected = null, draft = null, activeKey = null;
  let positions = new Map(), afterProfile = null, lastTime = 0, undo = null;
  let storageWarning = '';
  try { shared = C.flatten(window.PAPER_ANNOTATIONS || []); } catch (e) { storageWarning = '仓库批注格式有误：' + e.message; }
  try {
    author = localStorage.getItem(PROFILE) || '';
    if (!C.authorPattern.test(author)) author = '';
    const saved = JSON.parse(localStorage.getItem(KEY) || '[]');
    if (!Array.isArray(saved)) throw new Error('格式无效');
    local = C.flatten(saved);
  } catch (_) { storageWarning = '无法读取本地批注。旧数据未覆盖，请先检查浏览器存储或恢复导出的备份。'; }
  function notify(message, action = null) {
    const el = $('annotation-notice');
    el.querySelector('span').textContent = message;
    const button = el.querySelector('button');
    button.hidden = !action; button.onclick = action; el.hidden = false;
    clearTimeout(notify.timer);
    notify.timer = setTimeout(() => { el.hidden = true; }, 12000);
  }
  function rows() { return C.merge(shared, local); }
  function visible() { return rows().filter(a => !a.deleted && (filter === 'all' || (filter === 'mine' ? a.author === author : a.author !== author))); }
  function serialize(data) { return [...new Set(data.map(a => a.author))].sort().map(name => C.bundle(name, data)); }
  function persist(data) {
    if (storageWarning) { notify(storageWarning + ' 请先导出备份；刷新或恢复存储后重试。'); return false; }
    try {
      const latest = C.flatten(JSON.parse(localStorage.getItem(KEY) || '[]'));
      data = C.merge(latest, data);
      localStorage.setItem(KEY, JSON.stringify(serialize(data))); local = data; return true;
    }
    catch (_) { notify('保存失败，可能是浏览器存储已满或不可用。当前输入仍保留，请备份后重试。'); return false; }
  }
  function revision(previous) {
    lastTime = Math.max(Date.now(), lastTime + 1, Date.parse(previous || 0) + 1 || 0);
    return new Date(lastTime).toISOString();
  }
  function commit(a) {
    if (!persist(C.merge(local, [{...a, updatedAt: revision(a.updatedAt)}]))) return false;
    render(); return true;
  }
  function element(tag, cls, text) {
    const el = document.createElement(tag); if (cls) el.className = cls;
    if (text !== undefined) el.textContent = text; return el;
  }
  function button(text, action, cls) {
    const el = element('button', cls, text); el.type = 'button'; el.addEventListener('click', action); return el;
  }
  function sectionName(id) { return scopes.get(id)?.querySelector('h2,h1')?.textContent || '原文位置已变更'; }
  function textIndex(scope) {
    const nodes = []; let text = '';
    const walker = document.createTreeWalker(scope, NodeFilter.SHOW_TEXT, {acceptNode(node) {
      return node.parentElement.closest('button,script,style,[hidden],.annotation-ui') ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT;
    }});
    for (let node; (node = walker.nextNode());) { nodes.push({node, start: text.length, end: text.length + node.length}); text += node.data; }
    return {nodes, text};
  }
  function clearMarks() {
    article.querySelectorAll('mark[data-annotations]').forEach(mark => mark.replaceWith(...mark.childNodes));
    article.normalize();
  }
  function paint() {
    clearMarks(); positions = new Map();
    const shown = visible();
    for (const [section, scope] of scopes) {
      const index = textIndex(scope);
      const spans = [];
      for (const a of shown.filter(a => a.anchor.section === section)) {
        const pos = C.locate(index.text, a.anchor);
        if (pos) { const span = {...pos, key: C.key(a), mine: a.author === author}; spans.push(span); positions.set(span.key, span); }
      }
      // Split each text node into disjoint intervals. Overlapping authors share a mark.
      for (const entry of index.nodes) {
        const touching = spans.filter(s => s.start < entry.end && s.end > entry.start);
        if (!touching.length) continue;
        const cuts = [...new Set([entry.start, entry.end, ...touching.flatMap(s => [Math.max(entry.start, s.start), Math.min(entry.end, s.end)])])].sort((a, b) => a - b);
        const frag = document.createDocumentFragment();
        for (let i = 0; i < cuts.length - 1; i++) {
          const text = entry.node.data.slice(cuts[i] - entry.start, cuts[i + 1] - entry.start);
          const owners = touching.filter(s => s.start <= cuts[i] && s.end >= cuts[i + 1]);
          if (!owners.length) frag.append(document.createTextNode(text));
          else {
            const mark = element('mark', owners.some(s => s.mine) ? 'annotation-mark mine' : 'annotation-mark others', text);
            mark.dataset.annotations = owners.map(s => s.key).join(' '); mark.tabIndex = 0;
            mark.setAttribute('role', 'button'); mark.setAttribute('aria-label', '查看批注：' + text.slice(0, 60));
            mark.classList.toggle('annotation-active', owners.some(s => s.key === activeKey)); frag.append(mark);
          }
        }
        entry.node.replaceWith(frag);
      }
    }
  }
  function render() {
    paint(); list.replaceChildren();
    $('annotation-author').textContent = author ? '@' + author : '尚未设置作者';
    $('annotation-profile').textContent = author ? '更改' : '设置';
    const all = rows().filter(a => !a.deleted), shown = visible();
    $('annotation-count').textContent = shown.length;
    $('annotation-toggle-count').textContent = all.length || '';
    document.querySelectorAll('[data-filter]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.filter === filter)));
    $('annotation-sync-state').textContent = `仓库 ${shared.filter(a => !a.deleted).length} 条 · 本地内容需导出后才能通过 GitHub 共享。`;
    if (!shown.length) {
      const empty = element('div', 'annotation-empty');
      empty.append(element('span', 'annotation-empty-symbol', '〰'), element('strong', '', filter === 'others' ? '还没有他人的批注' : filter === 'mine' && !author ? '先设置你的署名' : '让想法留在原文旁'), element('p', '', filter === 'others' ? '拉取包含批注的仓库并构建，或导入对方的批注文件。' : '选中正文中的一段文字，即可高亮或记下笔记。'));
      if (!author) empty.append(button('设置我的署名', () => profile()));
      list.append(empty);
    }
    shown.sort((a, b) => {
      const order = [...scopes.keys()];
      return order.indexOf(a.anchor.section) - order.indexOf(b.anchor.section) || a.anchor.start - b.anchor.start || a.updatedAt.localeCompare(b.updatedAt);
    }).forEach(a => {
      const key = C.key(a), card = element('section', 'annotation-card'); card.dataset.key = key;
      card.classList.toggle('is-active', key === activeKey);
      const meta = element('div', 'annotation-meta');
      meta.append(element('strong', '', a.author === author ? '我 · @' + a.author : '@' + a.author));
      const time = element('time', '', new Date(a.updatedAt).toLocaleDateString('zh-CN', {month: 'numeric', day: 'numeric'})); time.dateTime = a.updatedAt; meta.append(time);
      const quote = button(a.anchor.exact, () => locateCard(key), 'annotation-card-quote');
      quote.title = '定位到原文';
      card.append(meta, element('div', 'annotation-section-name', sectionName(a.anchor.section)), quote);
      if (a.note) card.append(element('p', 'annotation-note-text', a.note));
      else card.append(element('p', 'annotation-highlight-only', '仅高亮'));
      if (!positions.has(key)) card.append(element('p', 'annotation-unmatched', '原文已变更，暂未定位；笔记仍保留。'));
      if (a.author === author) {
        const actions = element('div', 'annotation-card-actions');
        actions.append(button('编辑', () => edit(a)), button('删除', () => {
          if (commit({...a, deleted: true})) { undo = a; notify('已删除此批注。导出时会保留删除记录。', () => { if (undo) { const old = undo; undo = null; commit({...old, deleted: false, updatedAt: rows().find(r => C.key(r) === key)?.updatedAt}); notify('批注已恢复。'); } }); }
        }));
        if (!positions.has(key)) actions.append(button('重新关联选文', () => { if (selected) edit({...a, anchor: selected}); else notify('请先选中正文中对应的文字，再点击“重新关联选文”。'); }));
        card.append(actions);
      }
      list.append(card);
    });
  }
  function setOpen(open) {
    panel.hidden = !open; document.body.classList.toggle('annotations-open', open);
    toggle.setAttribute('aria-expanded', String(open));
  }
  function locateCard(key) {
    activeKey = key; setOpen(true);
    for (const mark of article.querySelectorAll('mark[data-annotations]')) mark.classList.toggle('annotation-active', mark.dataset.annotations.split(' ').includes(key));
    for (const card of list.children) card.classList.toggle('is-active', card.dataset.key === key);
    const mark = [...article.querySelectorAll('mark[data-annotations]')].find(m => m.dataset.annotations.split(' ').includes(key));
    if (!mark) { notify('没有找到原文位置。批注内容仍完整保留。'); return; }
    for (let parent = mark.parentElement; parent && parent !== article; parent = parent.parentElement) if (parent.tagName === 'DETAILS') parent.open = true;
    if (innerWidth < 1200) setOpen(false);
    mark.scrollIntoView({block: 'center', behavior: 'instant'});
    mark.focus({preventScroll: true});
  }
  function openMark(mark) {
    if (window.getSelection()?.toString()) return;
    const keys = mark.dataset.annotations.split(' ');
    activeKey = keys[(keys.indexOf(activeKey) + 1) % keys.length];
    setOpen(true); render();
    const card = [...list.children].find(c => c.dataset.key === activeKey);
    card?.scrollIntoView({block: 'nearest', behavior: 'instant'});
    card?.querySelector('button')?.focus({preventScroll: true});
  }
  article.addEventListener('click', e => { const mark = e.target.closest('mark[data-annotations]'); if (mark) openMark(mark); });
  article.addEventListener('keydown', e => { const mark = e.target.closest('mark[data-annotations]'); if (mark && ['Enter', ' '].includes(e.key)) { e.preventDefault(); openMark(mark); } });
  function captureSelection() {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed || !selection.rangeCount) { toolbar.hidden = true; return; }
    const range = selection.getRangeAt(0);
    if (!article.contains(range.startContainer) || !article.contains(range.endContainer)) { toolbar.hidden = true; return; }
    const entry = [...scopes].find(([, el]) => el.contains(range.startContainer) && el.contains(range.endContainer));
    if (!entry) { toolbar.hidden = true; notify('请在同一章节内选择文字，再添加批注。'); return; }
    const [section, scope] = entry, index = textIndex(scope);
    function offset(container, n) {
      if (container.nodeType === Node.TEXT_NODE) {
        const item = index.nodes.find(e => e.node === container); return item ? item.start + n : null;
      }
      const before = document.createRange(); before.setStart(scope, 0); before.setEnd(container, n);
      // The page's selectable scopes contain only textual content; controls are excluded above.
      return index.nodes.filter(e => before.intersectsNode(e.node)).reduce((sum, e) => sum + e.node.length, 0);
    }
    const start = offset(range.startContainer, range.startOffset), end = offset(range.endContainer, range.endOffset);
    if (start === null || end === null || end <= start || !index.text.slice(start, end).trim()) { toolbar.hidden = true; return; }
    if (end - start > 10000) { toolbar.hidden = true; notify('一次请选择不超过 10,000 字的一段正文。'); return; }
    selected = {section, start, exact: index.text.slice(start, end), prefix: index.text.slice(Math.max(0, start - 48), start), suffix: index.text.slice(end, end + 48)};
    const rect = range.getBoundingClientRect(); toolbar.hidden = false;
    toolbar.style.left = Math.max(8, Math.min(innerWidth - 190, rect.left + rect.width / 2 - 85)) + 'px';
    toolbar.style.top = Math.max(8, Math.min(innerHeight - 48, rect.top > 55 ? rect.top - 45 : rect.bottom + 8)) + 'px';
  }
  document.addEventListener('pointerup', e => { if (article.contains(e.target)) setTimeout(captureSelection, 0); });
  document.addEventListener('keyup', e => { if (e.key.startsWith('Arrow') || e.key === 'Shift') captureSelection(); });
  document.addEventListener('selectionchange', () => { if (window.getSelection()?.isCollapsed) toolbar.hidden = true; });
  toolbar.addEventListener('pointerdown', e => e.preventDefault());
  window.addEventListener('scroll', () => { toolbar.hidden = true; }, {passive: true});
  window.addEventListener('resize', () => { toolbar.hidden = true; });
  function profile(callback = null) {
    afterProfile = callback; $('annotation-username').value = author;
    $('annotation-profile-error').textContent = ''; $('annotation-profile-dialog').showModal(); $('annotation-username').focus();
  }
  function needAuthor(action) { if (author) action(); else profile(action); }
  $('annotation-profile-form').addEventListener('submit', e => {
    e.preventDefault(); const name = $('annotation-username').value.trim().toLowerCase();
    if (!C.authorPattern.test(name)) { $('annotation-profile-error').textContent = '请使用 1–39 位字母、数字或短横线，首尾不能是短横线。'; return; }
    if (draft && draft.author !== name) { $('annotation-profile-error').textContent = '请先保存或取消正在编辑的批注，再更改署名。'; return; }
    try { localStorage.setItem(PROFILE, name); } catch (_) { $('annotation-profile-error').textContent = '浏览器无法保存署名，请检查存储设置。'; return; }
    author = name; $('annotation-profile-dialog').close(); render(); const action = afterProfile; afterProfile = null; action?.();
  });
  $('annotation-profile-cancel').onclick = () => { afterProfile = null; $('annotation-profile-dialog').close(); };
  $('annotation-profile').onclick = () => profile();
  function storeDraft() {
    try { if (draft) localStorage.setItem(DRAFT, JSON.stringify(draft)); else localStorage.removeItem(DRAFT); }
    catch (_) { notify('草稿暂时无法自动保存；请保留页面并完成保存或复制笔记。'); }
  }
  function edit(a) {
    if (draft) { notify('请先保存或取消当前笔记。'); $('annotation-note').focus(); return; }
    draft = structuredClone(a); setOpen(true); toolbar.hidden = true; window.getSelection()?.removeAllRanges();
    editor.hidden = false; $('annotation-quote').textContent = a.anchor.exact; $('annotation-note').value = a.note;
    $('annotation-editor-title').textContent = rows().some(r => C.key(r) === C.key(a)) ? '编辑批注' : '添加批注';
    storeDraft(); editor.scrollIntoView({block: 'nearest', behavior: 'instant'}); $('annotation-note').focus();
  }
  function newAnnotation(note) {
    if (!selected) return;
    const anchor = structuredClone(selected);
    needAuthor(() => {
      const a = {author, id: 'n-' + (crypto.randomUUID ? crypto.randomUUID() : Date.now().toString(36) + Math.random().toString(36).slice(2)), updatedAt: revision(), deleted: false, anchor, note: ''};
      if (note) edit(a);
      else { if (filter === 'others') filter = 'mine'; if (commit(a)) { toolbar.hidden = true; window.getSelection()?.removeAllRanges(); notify('高亮已保存到本地。'); } }
    });
  }
  $('annotation-highlight').onclick = () => newAnnotation(false);
  $('annotation-comment').onclick = () => newAnnotation(true);
  $('annotation-note').addEventListener('input', () => { if (draft) { draft.note = $('annotation-note').value; storeDraft(); } });
  function closeEditor() { draft = null; editor.hidden = true; storeDraft(); }
  $('annotation-cancel').onclick = closeEditor;
  editor.addEventListener('submit', e => {
    e.preventDefault(); if (!draft || draft.author !== author) return;
    draft.note = $('annotation-note').value;
    const latest = rows().find(a => C.key(a) === C.key(draft));
    if (latest && latest.updatedAt > draft.updatedAt) { notify('此条批注有更新。请复制当前草稿，取消后重新打开最新批注再编辑。'); return; }
    if (filter === 'others') filter = 'mine';
    if (commit(draft)) { closeEditor(); notify('批注已保存在本地。导出后可纳入 GitHub 同步。'); }
  });
  toggle.onclick = () => { setOpen(panel.hidden); if (!panel.hidden) $('annotation-close').focus(); };
  $('annotation-close').onclick = () => { setOpen(false); toggle.focus(); };
  document.querySelectorAll('[data-filter]').forEach(b => b.onclick = () => { filter = b.dataset.filter; render(); });
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape' && !$('annotation-profile-dialog').open && !$('figure-dialog').open) { toolbar.hidden = true; if (!panel.hidden) { setOpen(false); toggle.focus(); } }
  });
  $('annotation-export').onclick = () => needAuthor(() => {
    const blob = new Blob([JSON.stringify(C.bundle(author, rows()), null, 2) + '\n'], {type: 'application/json'});
    const url = URL.createObjectURL(blob), a = document.createElement('a'); a.href = url; a.download = author + '.json'; a.click(); setTimeout(() => URL.revokeObjectURL(url), 5000);
    notify('已导出我的批注。尚未保存的草稿不包含在导出文件中。');
  });
  $('annotation-import').onclick = () => $('annotation-file').click();
  $('annotation-file').addEventListener('change', async e => {
    const file = e.target.files[0]; if (!file) return;
    try {
      if (file.size > 15 * 1024 * 1024) throw new Error('文件超过 15 MB，请检查文件。');
      const data = C.validate(JSON.parse(await file.text()));
      const incoming = C.flatten([data]);
      if (persist(C.merge(local, incoming))) { render(); notify(`已导入 @${data.author} 的批注，保留较新的修改。署名不会自动切换。`); }
    } catch (err) { notify('未导入：' + err.message); }
    e.target.value = '';
  });
  window.addEventListener('storage', e => {
    if (e.key === KEY) { try { local = C.flatten(JSON.parse(e.newValue || '[]')); render(); } catch (_) { notify('另一窗口的批注数据无法读取，请先导出备份。'); } }
  });
  render(); setOpen(innerWidth >= 1200);
  if (storageWarning) notify(storageWarning);
  try {
    const savedDraft = JSON.parse(localStorage.getItem(DRAFT) || 'null');
    if (savedDraft && savedDraft.author === author) { C.validate(C.bundle(author, [savedDraft])); edit(savedDraft); notify('已恢复上次未保存的笔记。'); }
  } catch (_) { notify('未能恢复上次的草稿；已保存的批注不受影响。'); }
})();
