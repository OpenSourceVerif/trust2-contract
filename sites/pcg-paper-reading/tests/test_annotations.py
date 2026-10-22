import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import annotations


class AnnotationSyncTests(unittest.TestCase):
    def setUp(self):
        self.bundle = {'version': 1, 'document': 'pcg-paper', 'author': 'reader-a', 'annotations': [
            {'id': 'n-1', 'updatedAt': '2026-10-23T01:00:00.000Z', 'deleted': False, 'note': '笔记',
             'anchor': {'section': 'introduction', 'start': 0, 'exact': 'Rust', 'prefix': '', 'suffix': ''}}]}

    def test_import_preserves_deletions_and_independent_authors(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(annotations, 'DIRECTORY', Path(tmp) / 'authors'):
            source = Path(tmp) / 'export.json'
            source.write_text(json.dumps(self.bundle))
            target = annotations.import_bundle(source)
            newer = copy.deepcopy(self.bundle)
            newer['annotations'][0].update(deleted=True, updatedAt='2026-10-23T02:00:00.000Z')
            source.write_text(json.dumps(newer))
            annotations.import_bundle(source)
            source.write_text(json.dumps(self.bundle))
            annotations.import_bundle(source)
            self.assertTrue(json.loads(target.read_text())['annotations'][0]['deleted'])
            peer = copy.deepcopy(self.bundle)
            peer['author'] = 'reader-b'
            source.write_text(json.dumps(peer))
            annotations.import_bundle(source)
            self.assertEqual(len(annotations.load_bundles()), 2)

    def test_rejects_path_traversal_and_ambiguous_source_files(self):
        for name in ['../reader', 'reader/', 'UPPER']:
            with self.assertRaises(ValueError):
                annotations.validate({**self.bundle, 'author': name})
        with tempfile.TemporaryDirectory() as tmp, patch.object(annotations, 'DIRECTORY', Path(tmp)):
            (Path(tmp) / 'someone-else.json').write_text(json.dumps(self.bundle))
            with self.assertRaises(ValueError):
                annotations.load_bundles()


if __name__ == '__main__':
    unittest.main()
