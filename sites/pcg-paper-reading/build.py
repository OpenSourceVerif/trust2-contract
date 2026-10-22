"""Build the local, offline research site. No third-party dependencies."""
import json
import re
import shutil
import os
from html import escape as esc
from pathlib import Path
from annotations import load_bundles
from urllib.parse import urlsplit, unquote

ROOT = Path(__file__).resolve().parent
DIST = ROOT / 'dist'
PLAN = json.loads((ROOT / 'content/plan.json').read_text())
STAGES = PLAN['stages']
BOOK = ROOT.parents[1] / 'docs/specification-reference/book'


def items(values):
    return '<ul>' + ''.join(f'<li>{esc(v)}</li>' for v in values) + '</ul>'


def check(task, label=''):
    checked = ' checked' if task['done'] else ''
    return (f'<label class="check-label"><input type="checkbox" data-task="{task["id"]}"'
            f' data-default="{str(task["done"]).lower()}" aria-label="标记完成：{esc(task["title"])}"{checked}>'
            f'{esc(label)}</label>')


def workspace_navigation(prefix='', active=False):
    return f'''<a class="brand" href="{prefix}index.html">trust2-contract<small>研究空间</small></a>
<p class="nav-label">工作空间</p><nav aria-label="全站导航">
<a href="{prefix}index.html" class="{'active' if active else ''}">研究工作台</a>
<a href="{prefix}reference/zh/introduction.html">规约框架参考手册 ↗</a></nav>'''


def shell(title, body, prefix='', stage=None):
    links = ''.join(f'<a href="{prefix}index.html#{s["id"]}"'
                    f'{" class=active" if s is stage else ""}>{s["number"]}　{s["title"]}</a>' for s in STAGES)
    return f'''<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="description" content="trust2-contract 研究工作台：PCG 研究路线、任务记录与规约框架参考手册。">
<title>{esc(title)} · trust2-contract</title><link rel="stylesheet" href="{prefix}style.css"><link rel="stylesheet" href="{prefix}research.css">
<link rel="stylesheet" href="{prefix}workbench.css">
<script src="{prefix}research.js" defer></script></head><body class="research workbench{' task-overview' if stage else ''}">
<a class="skip" href="#content">跳到正文</a>
<div class="mobile-bar"><span>trust2-contract · Research</span><button id="menu" aria-controls="sidebar" aria-expanded="false">目录</button></div>
<aside class="sidebar" id="sidebar">{workspace_navigation(prefix, active=not stage)}
<p class="nav-label">PCG 研究路径</p><nav aria-label="研究阶段">{links}</nav>
<div class="rail-foot"><span>阅读资料</span><a href="{prefix}paper.html">PCG 论文阅读 ↗</a></div></aside>
<main id="content">{body}<footer><span>trust2-contract · Research Workspace</span><span>阅读内容逐步完善，研究安排随进展确认。</span></footer></main>
<div class="toast" id="save-message" role="status" aria-live="polite"></div></body></html>'''


def home():
    sections = []
    for s in STAGES:
        rows = []
        for i, t in enumerate(s['tasks'], 1):
            href = t.get('page')
            opening = f'<a class="task-link" href="{href}">' if href else '<div class="task-link task-placeholder">'
            closing = '</a>' if href else '</div>'
            arrow = '<span class="arrow" aria-hidden="true">↗</span>' if href else ''
            status = '已完成' if t['done'] else ('待开展' if href else '待规划')
            rows.append(f'''<li class="task-row" data-row="{t['id']}">{check(t)}
{opening}<span class="task-number">{s['number']}.{i:02}</span><span class="task-copy"><span class="task-title">{esc(t['title'])}</span><span class="task-description">{esc(t['summary'])}</span></span><span class="task-kind">{esc(t['kind'])}</span><span class="task-status" data-status="{t['id']}">{status}</span>{arrow}{closing}</li>''')
        count = sum(t['done'] for t in s['tasks'])
        sections.append(f'''<section class="stage" id="{s['id']}" data-stage="{s['id']}">
<div class="stage-heading"><span class="stage-number">{s['number']}</span><div><p class="section-kicker">{'第一步 / ANALYSIS' if s['number']=='01' else '第二步 / SPECIFICATION'}</p><h2>{esc(s['title'])}</h2></div><span class="stage-progress">{count} / {len(s['tasks'])} 完成</span></div>
<p class="stage-intro">{esc(s['intro'])}</p><ol class="task-list">{''.join(rows)}</ol>
</section>''')
    return shell('研究工作台', f'''<header class="plan-header"><div class="workspace-heading"><div><p class="eyebrow">RESEARCH WORKSPACE</p><h1>研究工作台</h1><p class="plan-description">组织研究任务与阅读记录，查阅规约框架参考手册。</p></div><a class="continue-task" id="continue-task" href="tasks/charon-paper.html"><span>继续阅读</span><strong id="continue-title">Charon–Aeneas 论文阅读</strong><span aria-hidden="true">→</span></a></div>
<h2 class="journey-heading">PCG 研究路线<span>两步走</span></h2>
<div class="journey" aria-label="PCG 研究的两步路线">
<a class="journey-step" href="#analysis"><span class="journey-number">01</span><span class="journey-copy"><span class="journey-label">第一步 · 分析与信息保持</span><strong>{esc(STAGES[0]['title'])}</strong><span class="journey-description">从论文与源码阅读入手</span></span><span class="journey-count" data-stage-summary="analysis">1 / 7 完成</span><span class="journey-link" aria-hidden="true">↘</span></a>
<span class="journey-arrow" aria-hidden="true">→</span>
<a class="journey-step" href="#specification"><span class="journey-number">02</span><span class="journey-copy"><span class="journey-label">第二步 · 参考 Prusti</span><strong>{esc(STAGES[1]['title'])}</strong><span class="journey-description">在 LLBC 上使用 PCG 分析结果</span></span><span class="journey-count" data-stage-summary="specification">0 / 3 完成</span><span class="journey-link" aria-hidden="true">↘</span></a>
</div></header>
<div class="plan-toolbar"><div class="progress-summary"><strong>PCG 研究任务</strong><progress id="overall-meter" value="1" max="10" aria-label="PCG 研究任务完成进度"></progress><span><span id="overall-progress">1 / 10</span> 已完成</span></div><details class="record-menu" id="record-menu"><summary>记录管理 <span aria-hidden="true">⌄</span></summary><div class="record-popover"><p>进度与笔记保存在当前浏览器。</p><button id="export-progress" class="text-button">导出研究记录 ↓</button><label class="text-button import-label">导入记录 ↑<input type="file" id="import-progress" accept="application/json,.json"></label><p>导入会更新匹配任务的进度与笔记。</p></div></details></div>
{''.join(sections)}<section class="reference-callout"><div><p class="section-kicker">REFERENCE BOOK</p><h2>规约框架参考手册</h2><p>查阅规约语言、用法与当前实现边界。</p></div><div><a href="reference/zh/introduction.html">简体中文 ↗</a><a href="reference/en/introduction.html">English ↗</a></div></section>''')


def task_page(s, t):
    i = s['tasks'].index(t)
    rows = []
    for item in t['todos']:
        guidance = ''
        if item.get('focus'):
            guidance = f'<div class="todo-guidance"><section>{items(item["focus"])}</section></div>'
        rows.append(f'<li><label><input type="checkbox" data-todo="{item["id"]}" data-parent="{t["id"]}"><span>{esc(item["title"])}</span></label>{guidance}</li>')
    todos = ''.join(rows)
    reference = '<a href="../paper.html">查看 PCG 论文阅读页面 ↗</a>' if t['id'] == 'charon-paper' else ''
    return shell(t['title'], f'''<a class="breadcrumb" href="../index.html#{s['id']}">← PCG 研究路线 / {s['number']}</a>
<header class="task-header"><p class="eyebrow">{s['number']}.{i+1:02} / {esc(t['kind'])}</p><h1>{esc(t['title'])}</h1><div class="task-controls">{check(t, '标记为已完成')}<span class="task-status" data-status="{t['id']}">待开展</span></div></header>
<section class="reading-section reading-requirement"><h2>阅读要求</h2><p>{esc(t['requirement'])} {reference}</p><p class="replacement-note">阅读完成后，直接用整理好的阅读内容替换本页，而不是另写阅读记录。</p></section>
<section class="reading-section coarse-todos"><h2>阅读待办</h2><ul>{todos}</ul></section>
<div class="task-bottom"><a href="../index.html#{s['id']}">← 返回工作台</a></div>''', '../', s)


def build():
    DIST.mkdir(exist_ok=True)
    bundles = load_bundles()
    (DIST / 'annotations-data.js').write_text('window.PAPER_ANNOTATIONS = ' + json.dumps(bundles, ensure_ascii=True) + ';\n')
    source = (ROOT / 'content/pcg-paper.html').read_text()
    # Protect edits to the old entry page during the one-time migration.
    old = DIST / 'index.html'
    if old.exists() and '<title>Place Capability Graphs' in old.read_text() and old.read_text() != source:
        raise RuntimeError('The original paper changed after capture; refresh content/pcg-paper.html before building.')
    (DIST / 'index.html').write_text(home())
    (DIST / 'tasks').mkdir(exist_ok=True)
    for s in STAGES:
        for t in s['tasks']:
            target = DIST / 'tasks' / (t["id"] + ".html")
            if t.get('todos'):
                target.write_text(task_page(s, t))
            elif target.exists():
                target.unlink()
    paper = source.replace('</head>', '<link rel="stylesheet" href="research.css"><link rel="stylesheet" href="workbench.css"><link rel="stylesheet" href="paper.css"><link rel="stylesheet" href="annotations.css"><script src="annotations-data.js" defer></script><script src="annotation-core.js" defer></script><script src="annotations.js" defer></script></head>', 1)
    paper = paper.replace('<body id="top">', '<body id="top" class="research workbench paper-reading">', 1)
    chapters = re.search(r'<nav aria-label="章节目录">.*?</nav>', source, re.S).group(0)
    paper_sidebar = f'''<aside class="sidebar" id="sidebar">{workspace_navigation()}
<p class="nav-label">PCG 论文阅读</p>{chapters}
<div class="rail-foot"><span>原文资料</span><a href="assets/pcg-paper.pdf" target="_blank" rel="noopener">论文 PDF ↗</a><a href="https://doi.org/10.1145/3763122" target="_blank" rel="noopener">ACM DOI ↗</a></div></aside>'''
    paper = re.sub(r'<aside class="sidebar" id="sidebar">.*?</aside>', lambda _: paper_sidebar, paper, count=1, flags=re.S)
    paper = paper.replace('<header>', '<nav class="paper-breadcrumb" aria-label="当前位置"><a href="index.html">研究工作台</a><span aria-hidden="true">/</span><a href="index.html#analysis">01 · PCG 分析</a><span aria-hidden="true">/</span><span aria-current="page">论文阅读</span></nav><article class="paper-document"><header>', 1)
    paper = paper.replace('</main>', '</article></main>', 1)
    annotation_ui = (ROOT / 'content/annotation-ui.html').read_text()
    toggle = '<button type="button" class="annotation-toggle" id="annotation-toggle" aria-controls="annotation-panel" aria-expanded="false"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M4 4h16v12H9l-5 4V4Z"/><path d="M8 8h8M8 12h5"/></svg>批注 <span id="annotation-toggle-count" class="annotation-toggle-count"></span></button>'
    paper = paper.replace('</body>', toggle + annotation_ui + '</body>', 1)
    (DIST / 'paper.html').write_text(paper)
    # Preserve all former index.html#chapter deep links.
    home_ids = set(re.findall(r'\bid="([^"]+)"', home()))
    anchors = [a for a in re.findall(r'\bid="([^"]+)"', source) if a not in home_ids]
    meta = {'stages': [{'id': s['id'], 'tasks': [{'id': t['id'], 'title': t['title'], 'done': t['done'], 'page': t.get('page'), 'todos': [item['id'] for item in t.get('todos', [])]} for t in s['tasks']]} for s in STAGES], 'paperAnchors': anchors}
    (DIST / 'plan-data.js').write_text('window.RESEARCH_PLAN = ' + json.dumps(meta, ensure_ascii=False) + ';\n')
    for page in [DIST / 'index.html', *sorted((DIST / 'tasks').glob('*.html'))]:
        prefix = '../' if page.parent.name == 'tasks' else ''
        page.write_text(page.read_text().replace(f'<script src="{prefix}research.js"', f'<script src="{prefix}plan-data.js" defer></script><script src="{prefix}research.js"'))
    # Publish a copy; the original mdBook remains the single reference source.
    for lang in ('en', 'zh'):
        target = DIST / 'reference' / lang
        shutil.copytree(BOOK / lang, target, dirs_exist_ok=True)
        for page in target.glob('*.html'):
            text = page.read_text()
            def relocate(match):
                href = match.group(1)
                url = urlsplit(href)
                if href == '/':
                    return 'href="../../index.html"'
                if url.scheme or url.netloc or not url.path.startswith('../../../../'):
                    return match.group(0)
                original = (BOOK / lang / unquote(url.path)).resolve()
                repository = BOOK.parents[2]
                relative = original.relative_to(repository)
                copied = DIST / 'reference/source' / relative
                copied.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(original, copied)
                new_href = os.path.relpath(copied, page.parent)
                if url.fragment:
                    new_href += '#' + url.fragment
                return f'href="{esc(new_href, quote=True)}"'
            text = re.sub(r'href="([^"]+)"', relocate, text)
            text = text.replace('</head>', '<link rel="stylesheet" href="../../reference-nav.css"></head>', 1)
            home_label = '返回研究主页' if lang == 'zh' else 'Research home'
            home_button = f'<a class="reference-home icon-button" href="../../index.html" title="{home_label}" aria-label="{home_label}"><svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m3 10 9-7 9 7v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1Z"/><path d="M9 21v-8h6v8"/></svg></a>'
            text = text.replace('<div class="left-buttons">', '<div class="left-buttons">' + home_button, 1)
            page.write_text(text)
    print('Built research workbench, 2 reading pages, preserved PCG paper, and bilingual reference book.')


if __name__ == '__main__':
    build()
