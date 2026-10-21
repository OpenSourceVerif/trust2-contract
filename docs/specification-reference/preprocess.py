#!/usr/bin/env python3
# coding: utf-8
"""mdBook preprocessor for shared contract syntax and stable rule anchors.

Page layout, navigation, fonts, themes, search and code highlighting are supplied
by mdBook. This preprocessor only expands {{#rule ...}} and {{#syntax ...}}.
"""
import json
import re
import sys
from html import escape
from pathlib import Path

from railroad import render as render_railroad

ROOT = Path(__file__).resolve().parent
GRAMMAR = json.loads((ROOT/'grammar.json').read_text())
EXTERNAL = {'IDENTIFIER', 'Type', 'Literal', 'Path', 'TUPLE_INDEX'}


def destination(name):
    if name in GRAMMAR:
        return str(Path(GRAMMAR[name]['chapter']).with_suffix('.html'))+'#grammar-'+name
    if name in EXTERNAL:
        return 'notations.html#rust-syntax'
    raise ValueError('Undefined nonterminal: '+name)


def render_tokens(source):
    rendered = []
    for part in re.split(r'("[^"]*"|\b[A-Za-z_][A-Za-z_0-9]*\b)', source):
        if part.startswith('"'):
            rendered.append('<span class="grammar-literal">'+escape(part[1:-1])+'</span>')
        elif part in GRAMMAR or part in EXTERNAL:
            rendered.append(f'<span class="grammar-text"><a href="{destination(part)}">{part}</a></span>')
        elif re.fullmatch('[A-Za-z_][A-Za-z_0-9]*', part):
            raise ValueError('Undefined grammar symbol: '+part)
        else:
            rendered.append(escape(part).replace('?', '<sup>?</sup>').replace('*', '<sup>*</sup>'))
    return ''.join(rendered)


def anchor(name):
    label = escape(name).replace('.', '<wbr>.')
    return (f'<div class="rule" id="r-{name}"><a class="rule-link" '
            f'href="#r-{name}" title="{name}"><span>[{label}]</span></a></div>\n\n')


def alternatives(source):
    """Break top-level alternatives only; quoted pipes remain terminals."""
    depth, current, result = 0, '', []
    for token in re.findall(r'"[^"]*"|[()]|\||[^"()|]+', source):
        if token == '(':
            depth += 1
        elif token == ')':
            depth -= 1
        if token == '|' and depth == 0:
            result.append(current.strip())
            current = ''
        else:
            current += token
    result.append(current.strip())
    return result


def syntax(names, chapter, prefix):
    productions, diagrams = [], []
    for name in names:
        rule = GRAMMAR[name]
        canonical = chapter == rule['chapter']
        rule_id = ('grammar-' if canonical else 'grammar-index-')+name
        railroad_id = ('railroad-' if canonical else 'railroad-index-')+name
        options = alternatives(rule['definition'])
        separator = ' | ' if all(x.startswith('"') and x.endswith('"') for x in options) else ' <br>\u00a0\u00a0\u00a0\u00a0| '
        rhs = separator.join(render_tokens(x) for x in options)
        productions.append(f'<span class="grammar-text grammar-production" id="{rule_id}" '
            f'onclick="show_railroad()"><a href="#{railroad_id}">{name}</a></span> → '
            f'<br>\u00a0\u00a0\u00a0\u00a0\u00a0\u00a0{rhs}')
        svg = render_railroad(name, rule['definition'], destination)
        if LANGUAGE == 'en':
            svg = svg.replace(' 的语法铁路图', ' syntax diagram')
        svg = svg.replace('railroad-'+name+'-title', 'railroad-'+Path(chapter).stem+'-'+name+'-title')
        diagrams.append(f'<div class="railroad-production" id="{railroad_id}">{svg}</div>')
    return (anchor(prefix+'.syntax')+'<div class="grammar-container">\n'
            +'<p><strong><sup>Syntax</sup></strong><br>\n'
            +'</p>\n<p>'.join(productions)+'</p>\n'
            +'<p><button class="grammar-toggle-railroad" type="button" title="Toggle railroad display" '
            +'onclick="toggle_railroad()">Show Railroad</button></p>\n</div>\n\n'
            +'<div class="grammar-railroad grammar-hidden">\n'
            +'\n'.join(diagrams)+'\n</div>\n\n')


def process(items):
    for item in items:
        if 'Chapter' not in item:
            continue
        chapter = item['Chapter']
        content = chapter['content']
        prefix_match = re.search(r'\{\{#rule ([a-z.-]+)\}\}', content)
        if prefix_match:
            prefix = prefix_match[1]
            path = chapter['source_path']
            content = re.sub(r'\{\{#syntax ([A-Za-z_0-9 ]+)\}\}',
                lambda m: syntax(m[1].split(), path, prefix), content)
            content = re.sub(r'\{\{#rule ([a-z.-]+)\}\}',lambda m: anchor(m[1]),content)
            chapter['content'] = content
        process(chapter.get('sub_items', []))


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == 'supports':
        sys.exit(0 if len(sys.argv)>2 and sys.argv[2]=='html' else 1)
    context, book = json.load(sys.stdin)
    LANGUAGE = context['config']['book']['language']
    process(book['items'])
    json.dump(book, sys.stdout, ensure_ascii=False)
