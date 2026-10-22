# trust2-contract 研究网站 · 维护说明

**网站首页：[打开研究工作台](dist/index.html)**

在主仓库中，统一从 `sites/index.html` 打开网站；它直接跳转到本目录的 `dist/index.html`。`pcg-paper-reading` 是沿用的目录名，首页是整个研究工作台。网站源码与页面均由 trust2-contract 主仓库管理。

主页的「继续阅读」指向已有页面的未完成阅读任务，导入导出位于「记录管理」菜单。原有 PCG 论文阅读位于 `dist/paper.html`；原 `index.html#capturing-rust` 等论文锚点自动转到对应章节。

## 信息架构

- PCG Research Plan：PCG 研究的两个阶段及各自的任务列表，属于整个研究工作台中的一条研究路线。
  - 第一阶段：实现 ULLBC → LLBC 的 PCG 分析。
  - 第二阶段：基于 LLBC 的 PCG 信息进行规约表达。
  - 01：PCG 论文阅读（首个已完成任务）、PCG 源码阅读、Charon–Aeneas 论文阅读、trust2-contract 源码阅读、分析方案、ULLBC 原型、LLBC 信息保持。
  - 02：Prusti 源码阅读、规约表达设计、最小案例验证。
- 未完成工作保留概括性的待办与必要的阅读方向，不预设详细执行步骤、产物和完成标准。trust2-contract 源码阅读的两个部分分别列出学习重点，用来说明阅读目的，具体方案仍待研究。
- 目前只有两份未完成的阅读页面：Charon–Aeneas 论文阅读（Charon、Aeneas 两项待办），以及 trust2-contract 源码阅读（cargo-verify 工具、Charon 源码两部分）。其他任务仅在工作台列出，不生成详情页。
- 原有任务 ID 与这两页的地址保留，以兼容本地阅读记录。
- 待办页只呈现明确的阅读要求与粗粒度 TODO，不提供阅读记录输入框。Charon–Aeneas 阅读参考现有 PCG 论文页面；阅读完成后，用整理好的内容替换对应页面。修改生成来源后再构建，避免只改 `dist` 被覆盖。
- Reference Book：既有中英文手册的静态副本。源文件仍在 `../../docs/specification-reference/`，不在这里重复编辑。

## 文件职责

```text
content/
  plan.json           研究阶段、稳定任务 ID、计划内容和初始状态
  pcg-paper.html      保留下来的完整 PCG 论文阅读正文
build.py              生成首页、任务页，接入论文与参考手册
dist/
  index.html          网站入口（生成）
  tasks/*.html        两份简短的阅读待办页（生成）
  paper.html          论文阅读（生成）
  plan-data.js        浏览器端任务元数据（生成）
  research.css/js     研究页面样式与交互（直接维护）
  workbench.css       主页的紧凑工作台布局（直接维护）
  paper.css           复用工作台外观的论文长文阅读样式
  reference-nav.css   参考手册左上角的主页图标样式
  style.css/app.js    原论文页面样式与交互
  assets/             原论文 PDF 与图片
  reference/en,zh/    参考手册发布副本（生成）
```

## 扩展与构建

在 `content/plan.json` 中添加或调整任务，然后执行 `python3 build.py`。任务 ID 应保持稳定，用于页面地址和本地记录。未确认的工作只写大致方向，详细安排需经用户确认后再加入。研究结论需要补充实际依据后再写入长期内容；已经确认的语言能力在 reference book 中维护。

论文正文在 `content/pcg-paper.html` 编辑，样式和图示仍共用原文件。参考手册先在原目录构建，再运行本站构建脚本同步。脚本不会修改原手册，也不会导入已删除的旧研究材料。

勾选状态和任务笔记使用当前浏览器的 localStorage；它们不会修改 `plan.json`，也不会自动跨设备同步。首页支持导出、导入 JSON。导入会更新匹配任务的进度和笔记。清理浏览器数据、切换 file/HTTP 地址或浏览器时，请先导出；`file://` 的存储行为取决于浏览器，必要时使用同一 HTTP 地址浏览。

本次工作以用户指定的本地文件为入口；未进行云端发布。

## 论文批注与 GitHub 同步

论文页支持选文高亮、右侧笔记、编辑与删除（可撤销），以及「全部 / 我的 / 他人」筛选。筛选同时影响正文高亮与侧栏；自己的标记为淡黄色，他人为淡蓝色。宽屏默认展开右栏，窄屏通过「批注」按钮打开抽屉。选文限定在同一章节内，可跨段落与行内格式；图片可继续放大，图片像素本身不支持批注。

首次记录时设置 GitHub 用户名作为署名；这不是 GitHub 登录，也不是权限验证。同一人在不同设备上使用相同署名。笔记写入浏览器 localStorage，编辑中的草稿也会恢复；未保存的草稿不包含在导出文件中。浏览器无法保存时会提示失败，不会假装已经同步。

共享步骤：

1. 在「同步与备份」选择「导出我的批注」。
2. 在本站目录执行 `python3 annotations.py /导出文件的绝对路径/your-name.json`，合并到 `content/annotations/pcg-paper/your-name.json` 并构建。
3. 按常规 GitHub 流程同步作者 JSON 与生成的 `dist/annotations-data.js`。他人拉取后运行 `python3 build.py` 并刷新页面。

网页不能直接修改仓库或自动推送 GitHub。也可直接在页面「导入批注」预览他人的文件；导入仅保存到浏览器，不会切换当前署名。所有作者的已共享批注随 `annotations-data.js` 加载，兼容直接打开本地 HTML，无需 fetch 或后端。作者文件格式、冲突合并与删除规则见 [批注数据说明](content/annotations/pcg-paper/README.md)。

相关文件：`content/annotation-ui.html` 是界面模板；`dist/annotations.css`、`dist/annotations.js` 是界面和存储交互；`dist/annotation-core.js` 是校验、合并和文本定位规则；`annotations.py` 负责仓库导入。`dist/paper.html` 和 `dist/annotations-data.js` 由构建生成。批注通过独立标记层呈现，原始论文 HTML 不改写。

验证：`node --test tests/annotations.test.cjs`；`python3 -m unittest discover -s tests -p 'test_*.py'`。
