# trust2-contract Reference

使用 **mdBook 0.5.4** 和 **Rust Reference 原版主题**构建的规约语言参考手册。语法、示例与当前实现边界集中在每个主题页。侧边栏参考 [Prusti user guide](https://prusti.github.io/prusti/user-guide/intro.html)：连续编号的章节树，子章节缩进并保持展开，不混入页内标题。

## 查看

用浏览器打开 [index.html](index.html)，默认进入[英文版](book/en/introduction.html)。[中文版](book/zh/introduction.html)独立生成；每页右上角的语言链接切换到另一版本的同一章。整套目录需要保持在一起；已生成的 `book/` 包含字体、样式、脚本、搜索索引和铁路图，阅读不需要安装工具或连接网络。原来的章节地址继续重定向到新页面，并保留 URL 片段。

## 修改与构建

要求 Python 3.9+ 和 mdBook **0.5.4**。mdBook 可从[官方 release](https://github.com/rust-lang/mdBook/releases/tag/v0.5.4)取得，或执行 `cargo install mdbook --version 0.5.4 --locked`。构建脚本依次使用 `MDBOOK` 环境变量、`.tools/mdbook` 或 PATH 中的程序；`.tools/` 不提交。

在本目录执行：

```sh
python3 build.py
python3 build.py --check
```

第一条生成中英文手册、注入语言切换链接、兼容旧页面地址，并检查本地链接、资源、锚点及重复 ID。第二条只检查已有生成文件。请将源码和更新后的 `book/` 一起保存；不要手动修改 `book/`。

| 文件 | 用途 |
| --- | --- |
| `book.toml` | mdBook 配置及主题资源 |
| `src/en/SUMMARY.md`、`src/zh/SUMMARY.md` | 两种语言的同构章节目录 |
| `src/en/*.md`、`src/zh/*.md` | 英文、中文的各章说明与示例 |
| `grammar.json` | 唯一的文法定义及规则所属章节 |
| `preprocess.py` | 把语法指令展开为 Rust Reference 格式的 HTML |
| `railroad.py` | 从同一份文法生成可点击的 SVG 铁路图 |
| `theme/reference.css`、`theme/reference.js` | 原版 Rust Reference 主题，见下面的来源说明 |
| `build.py`、`validate.py` | 构建、兼容入口和链接检查 |
| `book/en/`、`book/zh/` | 两版完整静态站点，各自拥有搜索索引 |
| `theme/manual.css` | 语言切换按钮的局部样式；不改动原版主题 |

两版保持相同文件名、章节层级、规则锚点和文法指令。新增章节时需同时更新两版；修改文法只编辑 `grammar.json`。默认 `book.toml` 构建英文，`build.py` 使用 mdBook 的环境变量覆盖生成中文。

章节支持 `{{#rule contract.function}}` 稳定规则锚点，以及 `{{#syntax FunctionContract Precondition}}` 文法块。规则锚点使用 `r-` 前缀；非终结符链接到对应章节的 `grammar-Name`。

文法使用紧凑 EBNF：`|` 表示选择，`?` 表示可选，`*` 表示重复。宏仍由 Rust 表达式解析器处理；展示文法不能取代类型规则、运算符优先级及前端提取边界。实现范围与测试覆盖分开说明。

## 写作约定

手册正文只描述工具、用法、语法和当前实现状态。不要加入本次文档修改的过程记录、核对来源说明，或“未重新运行安装、编译、验证”等工作汇报；这些信息只放在对话或变更说明中。中英文版本遵循同一约定。


规约语言按概念组织为 Lexical Rules、Logic Expressions、Function Contracts、Statement Annotations 和 Type Invariants。具体构造页依次给出 Syntax、语义、示例和实现状态；以 ACSL 1.23 的章节组织和定义式行文为参考，只描述本项目已有的构造。实现状态应明确区分宏解析、类型检查、前端提取、验证和测试覆盖，不能将标记识别写成完整语义支持。完整文法索引保留在独立章节中，与主题页共用 `grammar.json`。

## 主题来源与许可

视觉基准为 [Rust Reference / Statements](https://doc.rust-lang.org/stable/reference/statements.html)。页面布局、字体、目录、搜索、主题与代码高亮使用 mdBook 0.5.4 的原生输出；生成的基础 CSS 和 JS 文件名哈希与所参考的页面一致。

`theme/reference.css` 和 `theme/reference.js` 于 2026-10-21 从该页面引用的以下资源原样保存，未自行重写其样式或交互：

- [reference-bb0bdaf8.css](https://doc.rust-lang.org/stable/reference/theme/reference-bb0bdaf8.css)
- [reference-da60281b.js](https://doc.rust-lang.org/stable/reference/theme/reference-da60281b.js)

这些文件来自 [rust-lang/reference](https://github.com/rust-lang/reference)，其 MIT 和 Apache-2.0 许可保留在 `theme/LICENSE-MIT` 和 `theme/LICENSE-APACHE`。后续更新主题时请同时核对来源及 mdBook 版本。本站的契约文法和铁路图生成器独立维护；没有复制 Rust 语言说明正文。
