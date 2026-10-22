# trust2-contract 研究网站

**网站入口：[打开研究工作台](index.html)**

用浏览器打开本目录的 `index.html`，即可进入研究工作台。研究路线、论文阅读、源码阅读待办和规约框架参考手册均从工作台进入。

整个 `sites/` 目录属于 trust2-contract 主仓库，与项目代码一起通过 GitHub 同步。`docs/` 仍用于维护规约框架参考手册。

网站当前实现在 `pcg-paper-reading/` 中，目录名沿用最初的论文阅读项目；网站内容已扩展为研究工作台。

- [网站维护说明](pcg-paper-reading/README.md)：内容结构、构建方式、进度与批注管理。
- `pcg-paper-reading/content/`：研究计划和阅读内容源。
- `pcg-paper-reading/dist/`：浏览器实际加载的网页与资源。

更新内容后，在 `pcg-paper-reading/` 中执行 `python3 build.py`，再刷新网页。
