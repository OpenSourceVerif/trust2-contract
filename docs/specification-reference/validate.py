"""Check generated local resources, grammar links, fragments, and unique IDs."""
from collections import Counter
from html.parser import HTMLParser
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__()
        self.ids, self.links = [], []
        self.feed(path.read_text())

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if 'id' in attrs:
            self.ids.append(attrs['id'])
        for name in ('href', 'src', 'xlink:href'):
            if attrs.get(name):
                self.links.append(attrs[name])


def validate(root):
    paths = list((root/'book').rglob('*.html'))
    assert paths, 'No generated pages. Run python3 build.py first.'
    pages = {p.resolve(): Page(p) for p in paths}
    errors, checked = [], 0
    for source in (root/'src/en').glob('*.md'):
        translation = root/'src/zh'/source.name
        if not translation.exists():
            errors.append(f'Missing Chinese chapter: {source.name}')
            continue
        syntax = r'\{\{#syntax .*?\}\}'
        if re.findall(syntax, source.read_text()) != re.findall(syntax, translation.read_text()):
            errors.append(f'Bilingual grammar directives differ: {source.name}')
    for lang, other in [('en', 'zh'), ('zh', 'en')]:
        for source in (root/'src'/lang).glob('*.md'):
            if source.name == 'SUMMARY.md':
                continue
            page_path = root/'book'/lang/source.with_suffix('.html').name
            expected = f'../{other}/{page_path.name}'
            if page_path.resolve() not in pages or expected not in pages[page_path.resolve()].links:
                errors.append(f'Missing language switch: {lang}/{page_path.name}')
    for path, page in pages.items():
        duplicates = [key for key, count in Counter(page.ids).items() if count > 1]
        if duplicates:
            errors.append(f'{path.name}: duplicate IDs: {duplicates}')
        if '{{#' in path.read_text():
            errors.append(f'{path.name}: unexpanded preprocessor directive')
        for link in page.links:
            parts = urlsplit(link)
            if parts.scheme or parts.netloc:
                continue
            target = (path.parent/unquote(parts.path)).resolve() if parts.path else path
            checked += 1
            if not target.exists():
                errors.append(f'{path.name}: missing {link}')
            elif parts.fragment and target in pages and unquote(parts.fragment) not in pages[target].ids:
                errors.append(f'{path.name}: missing anchor {link}')
    if errors:
        raise SystemExit('\n'.join(errors))
    print(f'Checked {len(pages)} HTML pages and {checked} local links; all valid.')
