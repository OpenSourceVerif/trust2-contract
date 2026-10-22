const test = require('node:test');
const assert = require('node:assert/strict');
const C = require('../dist/annotation-core.js');
const anchor = {section: 'introduction', start: 0, exact: '借用', prefix: '可变', suffix: '规则'};
const row = {author: 'reader-a', id: 'n-1', updatedAt: '2026-10-23T00:00:00.000Z', deleted: false, anchor, note: '笔记'};
test('relocates changed offsets and repeated text using context', () => {
  assert.equal(C.locate('新增内容：共享借用规则，可变借用规则。', anchor).start, 14);
  assert.equal(C.locate('借用与借用', {...anchor, prefix: '', suffix: ''}), null);
  assert.equal(C.locate('不存在', anchor), null);
});
test('newer edits and deletions survive stale imports; authors remain independent', () => {
  const deleted = {...row, deleted: true, updatedAt: '2026-10-23T01:00:00.000Z'};
  assert.equal(C.merge([deleted], [row])[0].deleted, true);
  const merged = C.merge([row], [{...row, author: 'reader-b'}]);
  assert.equal(merged.length, 2);
  assert.equal(C.bundle('reader-a', merged).annotations.length, 1);
  assert.deepEqual(C.flatten([C.bundle('reader-a', [row])]), [row]);
});
test('rejects invalid files, oversized notes, duplicate IDs and invalid timestamps', () => {
  const b = C.bundle('reader-a', [row]);
  assert.equal(C.validate(b), b);
  for (const bad of [{...b, author: undefined}, {...b, author: '../other'}, {...b, document: 'wrong'}, {...b, annotations: [b.annotations[0], b.annotations[0]]}, {...b, annotations: [{...b.annotations[0], note: 'x'.repeat(20001)}]}, {...b, annotations: [{...b.annotations[0], updatedAt: 'later'}]}]) assert.throws(() => C.validate(bad));
});
test('export retains tombstones and removes embedded author fields', () => {
  const b = C.bundle('reader-a', [{...row, deleted: true}]);
  assert.equal(b.annotations[0].deleted, true);
  assert.equal('author' in b.annotations[0], false);
});
