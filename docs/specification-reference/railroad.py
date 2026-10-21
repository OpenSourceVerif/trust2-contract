# coding: utf-8
"""Render the book's small EBNF notation as accessible, linked SVG railroads."""
import re
from html import escape


class Parser:
    def __init__(self, source):
        self.tokens = re.findall(r'"[^"]*"|[A-Za-z_][A-Za-z_0-9]*|[()|?*]', source)
        self.pos = 0

    def peek(self):
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def take(self):
        value = self.peek()
        self.pos += 1
        return value

    def choice(self):
        options = [self.sequence()]
        while self.peek() == '|':
            self.take()
            options.append(self.sequence())
        return options[0] if len(options) == 1 else ('choice', options)

    def sequence(self):
        children = []
        while self.peek() not in (None, ')', '|'):
            token = self.take()
            if token == '(':
                item = self.choice()
                assert self.take() == ')'
            else:
                item = ('terminal', token[1:-1]) if token.startswith('"') else ('reference', token)
            if self.peek() in ('?', '*'):
                op = self.take()
                item = ('choice', [('empty', ''), item if op == '?' else ('loop', item)])
            children.append(item)
        return children[0] if len(children) == 1 else ('sequence', children)


def measure(node):
    kind, value = node
    if kind in ('terminal', 'reference'):
        return (max(48, len(value) * 8 + 24), 34, 17)
    if kind == 'empty':
        return (32, 22, 11)
    if kind == 'sequence':
        sizes = [measure(child) for child in value]
        baseline = max((s[2] for s in sizes), default=11)
        return (sum(s[0] for s in sizes) + 20 * max(0, len(sizes) - 1),
                baseline + max((s[1] - s[2] for s in sizes), default=11), baseline)
    if kind == 'choice':
        sizes = [measure(child) for child in value]
        return (max(s[0] for s in sizes) + 48, sum(s[1] for s in sizes) + 14 * (len(sizes) - 1), sizes[0][2])
    if kind == 'loop':
        w, h, y = measure(value)
        return (w + 40, h + 28, y)
    raise ValueError(kind)


def draw(node, x, y, link):
    kind, value = node
    w, h, baseline = measure(node)
    cy = y + baseline

    def path(d):
        return f'<path d="{d}"/>'

    if kind in ('terminal', 'reference'):
        radius = 16 if kind == 'terminal' else 3
        text = escape(value)
        shape = (f'<g class="{"nonterminal" if kind == "reference" else "terminal"}"><rect x="{x}" y="{y + 1}" width="{w}" height="32" rx="{radius}"/>'
                 f'<text x="{x + w / 2}" y="{cy + 4}" text-anchor="middle">{text}</text></g>')
        return f'<a href="{escape(link(value), quote=True)}">{shape}</a>' if kind == 'reference' else shape
    if kind == 'empty':
        return path(f'M{x} {cy} H{x+w}')
    if kind == 'sequence':
        result, cursor = '', x
        for i, child in enumerate(value):
            cw, ch, cb = measure(child)
            if i:
                result += path(f'M{cursor-20} {cy} H{cursor}')
            result += draw(child, cursor, cy-cb, link)
            cursor += cw + 20
        return result
    if kind == 'choice':
        result, cursor = '', y
        for child in value:
            cw, ch, cb = measure(child)
            line_y = cursor + cb
            result += path(f'M{x} {cy} H{x+12} V{line_y} H{x+24}')
            result += draw(child, x+24, cursor, link)
            result += path(f'M{x+24+cw} {line_y} H{x+w-12} V{cy} H{x+w}')
            cursor += ch + 14
        return result
    if kind == 'loop':
        cw, ch, cb = measure(value)
        result = path(f'M{x} {cy} H{x+20}') + draw(value, x+20, y, link)
        result += path(f'M{x+20+cw} {cy} H{x+w}')
        bottom = y+h-10
        result += path(f'M{x+w-7} {cy} V{bottom} H{x+7} V{cy}')
        # A left-pointing arrow on the return edge distinguishes repetition.
        middle = x+w/2
        result += path(f'M{middle+5} {bottom-4} L{middle} {bottom} L{middle+5} {bottom+4}')
        return result
    raise ValueError(kind)


def render(name, source, link):
    parser = Parser(source)
    node = parser.choice()
    assert parser.peek() is None, (name, parser.peek())
    w, h, baseline = measure(node)
    y = baseline+40
    return (f'<svg xmlns="http://www.w3.org/2000/svg" class="railroad" style="max-width:100%;height:auto" role="img" aria-labelledby="railroad-{name}-title" '
            f'viewBox="0 0 {w+64} {h+54}" width="{w+64}" height="{h+54}">'
            f'<title id="railroad-{name}-title">{escape(name)} 的语法铁路图</title>'
            f'<path d="M8 {y-6} V{y+6} M12 {y-6} V{y+6} M12 {y} H32"/>'
            + f'<a href="{escape(link(name), quote=True)}"><text class="comment" x="{len(name)*4+20}" y="20">{escape(name)}</text></a>'
            + draw(node, 32, 40, link)
            + f'<path d="M{w+32} {y} H{w+52} M{w+52} {y-6} V{y+6} M{w+56} {y-6} V{y+6}"/></svg>')
