'use strict';
const plan = window.RESEARCH_PLAN;
const tasks = plan.stages.flatMap(stage => stage.tasks);
const defaults = Object.fromEntries(tasks.map(task => [task.id, {done: task.done, note: '', todos: {}}]));
const storageKey = 'trust2-contract.research.v1';
let state = structuredClone(defaults);
let toastTimer;

// Keep the original paper bookmarks valid after promoting the research plan.
if (!location.pathname.includes('/tasks/') && plan.paperAnchors.includes(location.hash.slice(1))) {
  location.replace('paper.html' + location.hash);
}

function message(text) {
  const target = document.querySelector('#save-message');
  target.textContent = text;
  target.classList.add('visible');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => target.classList.remove('visible'), 4500);
}

function parseRecord(text) {
  const value = JSON.parse(text);
  if (value.version !== 1 || !value.tasks || typeof value.tasks !== 'object' || Array.isArray(value.tasks)) {
    throw new Error('Unsupported research record');
  }
  const result = {};
  for (const task of tasks) {
    if (!Object.hasOwn(value.tasks, task.id)) continue;
    const record = value.tasks[task.id];
    if (!record || typeof record.done !== 'boolean' || typeof record.note !== 'string') {
      throw new Error('Invalid task record');
    }
    const todos = Object.fromEntries((task.todos || []).map(id => [id, record.todos?.[id] === true]));
    result[task.id] = {done: record.done, note: record.note, todos};
  }
  if (!Object.keys(result).length) throw new Error('No matching tasks');
  return result;
}

try {
  const stored = localStorage.getItem(storageKey);
  if (stored) state = {...state, ...parseRecord(stored)};
} catch {
  message('未能读取本地记录，当前显示计划中的初始状态。');
}

function save() {
  try {
    localStorage.setItem(storageKey, JSON.stringify({version: 1, tasks: state}));
    return true;
  } catch {
    message('浏览器未能保存记录，请在关闭页面前从首页导出备份。');
    return false;
  }
}

function render() {
  document.querySelectorAll('[data-task]').forEach(input => {
    input.checked = state[input.dataset.task].done;
  });
  document.querySelectorAll('[data-status]').forEach(label => {
    const done = state[label.dataset.status].done;
    const task = tasks.find(t => t.id === label.dataset.status);
    label.textContent = done ? '已完成' : (task.page ? '待开展' : '待规划');
    label.classList.toggle('done', done);
  });
  for (const stage of plan.stages) {
    const count = stage.tasks.filter(task => state[task.id].done).length;
    const label = document.querySelector(`[data-stage="${stage.id}"] .stage-progress`);
    if (label) label.textContent = `${count} / ${stage.tasks.length} 完成`;
    const summary = document.querySelector(`[data-stage-summary="${stage.id}"]`);
    if (summary) summary.textContent = `${count} / ${stage.tasks.length} 完成`;
  }
  document.querySelectorAll('[data-todo]').forEach(input => {
    input.checked = state[input.dataset.parent].todos[input.dataset.todo] === true;
  });
  const completed = tasks.filter(task => state[task.id].done).length;
  const progress = document.querySelector('#overall-progress');
  if (progress) progress.textContent = `${completed} / ${tasks.length}`;
  const meter = document.querySelector('#overall-meter');
  if (meter) { meter.max = tasks.length; meter.value = completed; }
  const nextLink = document.querySelector('#continue-task');
  if (nextLink) {
    const next = tasks.find(task => task.page && !state[task.id].done);
    nextLink.hidden = !next;
    if (next) {
      nextLink.href = next.page;
      document.querySelector('#continue-title').textContent = next.title;
    }
  }
}

const recordMenu = document.querySelector('#record-menu');
document.addEventListener('click', event => {
  if (recordMenu?.open && !recordMenu.contains(event.target)) recordMenu.open = false;
});
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && recordMenu?.open) {
    recordMenu.open = false;
    recordMenu.querySelector('summary').focus();
  }
});

document.querySelectorAll('[data-task]').forEach(input => {
  input.addEventListener('change', () => {
    state[input.dataset.task].done = input.checked;
    const saved = save();
    render();
    if (saved) message('研究进度已保存');
  });
});

document.querySelectorAll('[data-todo]').forEach(input => {
  input.addEventListener('change', () => {
    state[input.dataset.parent].todos[input.dataset.todo] = input.checked;
    if (save()) message('阅读待办已保存');
  });
});

const note = document.querySelector('[data-note]');
if (note) {
  const status = document.querySelector('#note-status');
  note.value = state[note.dataset.note].note;
  status.textContent = note.value ? '已载入本地记录' : '尚无记录';
  note.addEventListener('input', () => {
    state[note.dataset.note].note = note.value;
    status.textContent = save() ? '已保存到当前浏览器' : '未能保存，请复制笔记备份';
  });
}

document.querySelector('#export-progress')?.addEventListener('click', () => {
  const blob = new Blob([JSON.stringify({version: 1, exportedAt: new Date().toISOString(), tasks: state}, null, 2)], {type: 'application/json'});
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = 'trust2-contract-research.json';
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  message('已导出进度与全部任务笔记');
});

document.querySelector('#import-progress')?.addEventListener('change', async event => {
  const file = event.target.files[0];
  if (!file) return;
  try {
    if (file.size > 5 * 1024 * 1024) throw new Error('Too large');
    const imported = parseRecord(await file.text());
    state = {...state, ...imported};
    const saved = save();
    render();
    if (saved) message('已导入记录，匹配的任务进度与笔记已更新');
  } catch {
    message('导入失败：请选择本站导出的研究记录 JSON（不超过 5 MB）。');
  }
  event.target.value = '';
});

// Refresh progress after returning from a task page, including back/forward cache.
function refresh() {
  try {
    const stored = localStorage.getItem(storageKey);
    if (stored) state = {...structuredClone(defaults), ...parseRecord(stored)};
    render();
    if (note && document.activeElement !== note) note.value = state[note.dataset.note].note;
  } catch { /* Keep the current unsaved view if storage is inaccessible. */ }
}
window.addEventListener('pageshow', refresh);
window.addEventListener('storage', event => { if (event.key === storageKey) refresh(); });

const menu = document.querySelector('#menu');
const sidebar = document.querySelector('#sidebar');
menu.addEventListener('click', () => {
  menu.setAttribute('aria-expanded', String(sidebar.classList.toggle('open')));
});
sidebar.querySelectorAll('a').forEach(link => link.addEventListener('click', () => {
  sidebar.classList.remove('open');
  menu.setAttribute('aria-expanded', 'false');
}));
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && sidebar.classList.contains('open')) {
    sidebar.classList.remove('open');
    menu.setAttribute('aria-expanded', 'false');
    menu.focus();
  }
});
render();
