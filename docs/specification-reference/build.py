#!/usr/bin/env python3
"""Build both languages with pinned mdBook, add language links, and validate."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

from validate import validate

ROOT = Path(__file__).resolve().parent
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
    # Plain links work with file:// as well as HTTP, without browser storage.
    for lang, other, label in [('en', 'zh', '简体中文'), ('zh', 'en', 'English')]:
        for page in (ROOT/'book'/lang).glob('*.html'):
            text = page.read_text()
            marker = '<div class="right-buttons">'
            link = f'<a class="language-switch" href="../{other}/{page.name}" lang="{other}" hreflang="{other}">{label}</a>'
            if marker in text:
                page.write_text(text.replace(marker, marker+link, 1))
    pages = ['index', 'print', 'toc', '404'] + [Path(p).stem for p in sorted(sources['en']) if p != 'SUMMARY.md']
    for page in pages:
        # Entry URLs under book/ redirect to English by default.
        redirect(ROOT/'book'/f'{page}.html', f'en/{page}.html')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='validate existing generated books')
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


if __name__ == '__main__':
    main()
