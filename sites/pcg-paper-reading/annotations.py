"""Versioned, per-author annotation snapshots shared through ordinary Git files."""
import json
import re
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DIRECTORY = ROOT / 'content/annotations/pcg-paper'


def validate(data):
    if (not isinstance(data, dict) or data.get('version') != 1 or
        data.get('document') != 'pcg-paper' or
        not re.fullmatch(r'[a-z0-9](?:[a-z0-9-]{0,37}[a-z0-9])?', str(data.get('author', ''))) or
        not isinstance(data.get('annotations'), list) or len(data['annotations']) > 10000):
        raise ValueError('Invalid annotation bundle')
    ids = set()
    for a in data['annotations']:
        if not isinstance(a, dict):
            raise ValueError('Invalid annotation')
        anchor = a.get('anchor')
        if (not re.fullmatch(r'[a-zA-Z0-9_-]{1,100}', str(a.get('id', ''))) or a['id'] in ids or
            not isinstance(a.get('updatedAt'), str) or
            not re.fullmatch(r'\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z', a['updatedAt']) or
            type(a.get('deleted')) is not bool or not isinstance(a.get('note'), str) or len(a['note']) > 20000 or
            not isinstance(anchor, dict)):
            raise ValueError('Invalid or duplicate annotation')
        datetime.fromisoformat(a['updatedAt'].replace('Z', '+00:00'))
        for field, limit in [('section', 100), ('exact', 10000), ('prefix', 64), ('suffix', 64)]:
            value = anchor.get(field)
            if not isinstance(value, str) or len(value.encode('utf-16-le')) // 2 > limit:
                raise ValueError('Invalid text anchor')
        if not anchor['exact'].strip() or type(anchor.get('start')) is not int or not 0 <= anchor['start'] <= 9007199254740991:
            raise ValueError('Invalid text anchor')
        ids.add(a['id'])
    return data


def load_bundles():
    bundles = []
    for path in sorted(DIRECTORY.glob('*.json')):
        data = validate(json.loads(path.read_text()))
        if path.stem != data['author']:
            raise ValueError(f'{path.name}: filename must match author')
        bundles.append(data)
    return bundles


def import_bundle(path):
    incoming = validate(json.loads(Path(path).read_text()))
    target = DIRECTORY / f'{incoming["author"]}.json'
    old = validate(json.loads(target.read_text())) if target.exists() else {'annotations': []}
    rows = {a['id']: a for a in old['annotations']}
    for a in incoming['annotations']:
        if a['id'] not in rows or a['updatedAt'] >= rows[a['id']]['updatedAt']:
            rows[a['id']] = a
    incoming['annotations'] = [rows[k] for k in sorted(rows)]
    DIRECTORY.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(incoming, ensure_ascii=False, indent=2) + '\n')
    temporary.replace(target)
    return target


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description='Merge an exported annotation JSON, then rebuild the static website.')
    parser.add_argument('file', type=Path)
    args = parser.parse_args()
    print('Merged:', import_bundle(args.file))
    from build import build
    build()
