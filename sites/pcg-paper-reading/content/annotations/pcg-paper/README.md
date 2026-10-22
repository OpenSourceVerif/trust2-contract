# PCG 论文批注

每位作者一份 `<github-username>.json`，作者名使用小写。此目录只保存明确导出并纳入仓库的批注，不包含浏览器中的未导出笔记。

在论文页「同步与备份」导出我的批注后，从站点根目录执行：

```sh
python3 annotations.py /绝对路径/your-name.json
```

脚本按批注 ID 与 `updatedAt` 合并修改，保留删除标记，然后重新构建。按常规 Git 流程提交这个目录的作者文件及生成的 `dist/annotations-data.js`（和其他需要同步的网站变更）。其他人拉取仓库后运行 `python3 build.py` 并刷新网页，即可显示批注。此脚本不会执行 Git 提交或推送。

作者名是协作署名，不是登录或访问控制。请只提交自己的文件；仓库权限与代码审查决定共享内容。跨设备使用相同用户名，同一作者多设备修改时，以时间较新的记录为准，请保持设备时间正确。相同时间的记录以导入文件为准。重要冲突可通过 Git 历史查阅恢复。

## 格式

```json
{
  "version": 1,
  "document": "pcg-paper",
  "author": "your-name",
  "annotations": [{
    "id": "n-stable-unique-id",
    "updatedAt": "2026-10-23T00:00:00.000Z",
    "deleted": false,
    "note": "笔记内容，也可以为空字符串，仅保留高亮",
    "anchor": {
      "section": "introduction",
      "start": 0,
      "exact": "选中的文字",
      "prefix": "前文最多 48 个 UTF-16 单元",
      "suffix": "后文最多 48 个 UTF-16 单元"
    }
  }]
}
```

`section` 对应章节 ID，论文开头使用 `__header`；`start` 是该章节纯文本的 UTF-16 偏移。定位优先匹配选文与上下文，重复且无法唯一判断时标记为未定位。正文调整后不会删除笔记。自己的未定位批注可先在正文重新选文，再点击「重新关联选文」并保存。

不要物理删除已共享的条目：用 `deleted: true` 与新的时间保存删除记录，以免旧的浏览器副本或导出文件重新引入它。
