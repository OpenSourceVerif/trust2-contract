#!/usr/bin/env python3
"""Build both languages with pinned mdBook, add language links, and validate."""
import argparse
from html import escape
import os
from pathlib import Path
import re
import shutil
import subprocess
from urllib.parse import quote, unquote, urlsplit

from validate import validate

ROOT = Path(__file__).resolve().parent
REPO_ROOT = ROOT.parents[1]
VERSION = '0.5.4'


def redirect(path, target):
    path.write_text(f'''<!doctype html>
<html lang="en"><meta charset="utf-8"><title>trust2-contract Reference</title>
<meta http-equiv="refresh" content="0;url={target}">
<script>location.replace("{target}" + location.search + location.hash);</script>
<p><a href="{target}">Open the manual</a></p></html>
''')


def build(executable):
    sources = {lang: {p.name for p in (ROOT/'src'/lang).glob('*.md')} for lang in ('en', 'zh')}
    if sources['en'] != sources['zh']:
        raise SystemExit('English and Chinese chapter files must match.')
    for lang, html_lang in [('en', 'en'), ('zh', 'zh-CN')]:
        env = dict(os.environ, MDBOOK_BOOK__SRC=f'src/{lang}', MDBOOK_BOOK__LANGUAGE=html_lang)
        subprocess.run([executable, 'build', str(ROOT), '--dest-dir', str(ROOT/'book'/lang)],
                       check=True, cwd=ROOT, env=env)
    # Plain links work with file://, HTTP subpaths (such as GitHub Pages), and local servers.
    for lang, other, label in [('en', 'zh', '简体中文'), ('zh', 'en', 'English')]:
        for page in (ROOT/'book'/lang).glob('*.html'):
            text = page.read_text().replace('href="/"', 'href="index.html"')
            marker = '<div class="right-buttons">'
            link = f'<a class="language-switch" href="../{other}/{page.name}" lang="{other}" hreflang="{other}">{label}</a>'
            if marker in text:
                text = text.replace(marker, marker+link, 1)
            page.write_text(text)
    pages = ['index', 'print', 'toc', '404'] + [Path(p).stem for p in sorted(sources['en']) if p != 'SUMMARY.md']
    for page in pages:
        # Entry URLs under book/ redirect to English by default.
        redirect(ROOT/'book'/f'{page}.html', f'en/{page}.html')
    (ROOT/'book'/'.nojekyll').write_text('')


def rewrite_repo_links(blob_base):
    base = blob_base.rstrip('/')
    for lang in ('en', 'zh'):
        for page in (ROOT/'book'/lang).glob('*.html'):
            def replace_href(match):
                href = match.group(1)
                parts = urlsplit(href)
                if parts.scheme or parts.netloc or not parts.path.startswith('../../../../'):
                    return match.group(0)
                target = (page.parent / unquote(parts.path)).resolve()
                relative = target.relative_to(REPO_ROOT).as_posix()
                url = f"{base}/{quote(relative, safe='/')}"
                if parts.fragment:
                    url += f'#{parts.fragment}'
                return f'href="{escape(url, quote=True)}"'

            updated = re.sub(r'href="([^"]+)"', replace_href, page.read_text())
            page.write_text(updated)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='validate existing generated books')
    parser.add_argument(
        '--repo-blob-base',
        metavar='URL',
        help='rewrite relative repository file links to <URL>/<path> after local validation (e.g. for GitHub Pages)',
    )
    args = parser.parse_args()
    if not args.check:
        local = ROOT/'.tools/mdbook'
        executable = os.environ.get('MDBOOK') or (str(local) if local.exists() else shutil.which('mdbook'))
        if not executable:
            parser.error('Install mdBook 0.5.4 (see README.md), or set MDBOOK to its executable.')
        version = subprocess.check_output([executable, '--version'], text=True).strip()
        if version != f'mdbook v{VERSION}':
            parser.error(f'Expected mdbook v{VERSION}; found {version}')
        build(executable)
    validate(ROOT)
    if args.repo_blob_base:
        rewrite_repo_links(args.repo_blob_base)


if __name__ == '__main__':
    main()
