# THIR 规格提取原型

本原型已经跑通 Rust → THIR 逻辑项 + 程序 LLBC → WhyML → Z3。普通函数和 `#[pure]` 函数的程序实现继续经过现有 MIR/Charon/WhyML 流程；前后置条件、行内断言和量词从 THIR 提取，独立保存为逻辑项。

它回答了之前讨论的一个具体问题：可以在 THIR 路线上保留 Prusti 式的用户体验——同一个 `#[pure]` Rust 函数既用于程序，也用于规格。这里没有引入第二套表层表达式语言。

## 怎么运行

仓库使用 `nightly-2026-06-01`。运行环境需要 `why3find` 和 Z3。当前机器上的命令：

```sh
opam exec --switch=trust2-contract -- bash examples/thir-spec-prototype/run.sh good
opam exec --switch=trust2-contract -- bash examples/thir-spec-prototype/run.sh bad
opam exec --switch=trust2-contract -- bash examples/thir-spec-prototype/run.sh impure
```

`good` 应成功；`bad`、`impure` 应返回非零状态。脚本会构建驱动，输出保存在 `target/thir-prototype/<模式>/`：

- `spec/*.thir-spec.json`：独立逻辑项。
- `llbc/*.llbc`：擦除规格后的程序。
- `whyml/*.mlw`：程序与规格合并后的验证输入。
- `whyml/why3find.proofs/`：证明记录。

显式设置环境变量 `TRUST2_THIR_SPEC_DIR` 启用原型，它的值是逻辑项输出目录。未设置时走原有规格提取流程。这个开关暂时通过环境变量传递，避免为原型改动 Charon 的选项格式。

不启用 `verify` feature 时，`#[pure]` 是恒等属性宏，函数照常编译成可执行 Rust；规格宏仍按原来的方式擦除。

## 已运行的例子与结果

例子在 [`examples/thir-spec-prototype/src/lib.rs`](../examples/thir-spec-prototype/src/lib.rs)。

| 场景 | 结果 |
| --- | --- |
| `maximum(a, b)`：带 `if` 的可执行纯函数 | 程序实现与 THIR 定义一致性的后置条件得到证明 |
| `nonnegative(a)`：局部 `let`，调用另一个纯函数 | 得到证明 |
| `choose`：前后置条件、引用局部变量的断言、变量遮蔽 | 得到证明 |
| 在 `forall!`、`exists!` 中调用 `maximum` | 得到证明 |
| 手写 `#[pure] PartialEq::eq`，只比较 `key`，忽略 `payload` | 程序调用和规格调用使用相同的方法体，得到证明 |
| 在已知非负的局部值上增加 `value < 0` | 验证失败，其他函数仍得到证明 |
| 在规格中调用未标记纯性的 `ordinary` | THIR 提取报错，指出非纯调用 |

正例的 5 个函数验证目标全部通过，证明器为 Z3 4.13.3。错误断言例中 `choose` 的验证条件拆分后留下一个未证明目标。正例 LLBC 的 `spec_closures`、`spec_bodies` 长度均为 0。

另外运行了不启用 `verify` 的 `cargo check`、原型与 `cargo-verify` 的 Clippy，以及 Charon 的 `make clippy`。这些检查通过。Charon 的完整 `make test` 完成 Rust 测试目标的编译后，停在 OCaml 构建：当前 opam 环境缺少 `unionFind`、`visitors.ppx`、`ppx_deriving.show`。

关闭原型开关后也运行了已有的 `simple` 例子，沿用 MIR 规格路径。当前机器仅配置 Z3；`max`、`square` 通过，`strict_sub` 留下一个未证明目标。这个额外检查没有达到仓库 `result.txt` 记录的全部通过状态。

## 实现结构

```text
Rust 宏展开、类型检查
        │
        ├─ THIR ── 规格 / pure 函数 ── 独立逻辑项 JSON ─────────┐
        │                                                  │
        └─ mir_built                                      │
             ├─ 擦除规格闭包与捕获                          │
             ├─ 保留初始化检查 FakeRead                     │
             └─ 插入 thir_anchor(id)                        │
                    │                                      │
              borrowck → Charon ULLBC/LLBC                  │
                    │                                      │
                    └──────────── WhyML 后端 ──────────────┘
                                      │
                                      Z3
```

- [`spec-prototype`](../spec-prototype/src/lib.rs) 定义不含 rustc 生命周期的逻辑项与交换格式。
- [`spec-prototype-rustc`](../spec-prototype-rustc/src/lib.rs) 完成 THIR 提取、纯调用检查、方法解析、变量绑定记录和 `mir_built` 擦除。
- [`ctx/thir_spec.rs`](../cargo-verify/src/llbc_to_whyml/ctx/thir_spec.rs) 将逻辑项直接编码为 WhyML `Term`，并处理 anchor。
- Charon 驱动增加两个接入点：在程序翻译前提取 THIR，以及覆盖 `mir_built` provider。没有修改 Charon AST 或 LLBC 序列化格式。

目前仍调用 rustc 原始的 `mir_built` provider，然后改写返回值。因此初始 MIR 会产生；规格闭包不再进入后续规格 LLBC 编码。这个原型实施的是“早提取、早擦除”，没有替换 rustc 的 MIR builder。

## `#[pure]` 的实际含义

`#[pure]` 在验证模式下展开为驱动识别的属性标记。检查器翻译该函数的 THIR，并仅接受支持的纯表达式形式；对调用使用 rustc 的 `Instance::try_resolve` 取得具体方法实现，再要求它是本 crate 中标记了 `#[pure]` 的函数。赋值、循环、间接调用、未标记函数等会报错。

原型把逻辑调用展开成显式 `let` 绑定参数后的函数体。例如 `maximum(x, y)` 生成：

```text
let a = x in
let b = y in
if a >= b then a else b
```

与此同时，程序侧的 `maximum` 仍翻译其 MIR/LLBC 实现，并添加 `result = <THIR 逻辑定义>` 的后置条件。验证器检查程序实现满足这一定义，程序调用点使用这个后置条件。这使原型无需先实现独立逻辑函数声明和跨模块函数链接。

比较运算保留 Rust 的类型及方法解析：内建整数比较沿用既有 `I32` 编码；用户定义的 `PartialEq::eq` 展开其已检查的纯函数体。例子中的相等只比较 `key`，不会换成结构相等。

这里有一个具体的分派限制：`a: &Key`、`b: &Key` 时，`a == b` 会先调用标准库的引用转发实现，它尚未纳入纯函数库。例子使用 `*a == *b`，直接解析到用户标记的实现。

## Anchor 与变量绑定

`thir_anchor(id)` 的 `id` 是本次编译内一条规格的编号，由规格闭包的 `LocalDefIndex` 生成。它不表示 MIR local。整条 marker 调用在程序中的位置表示断言发生的位置；后端见到它时，根据 id 查独立规格表。

变量身份另行保存：

1. THIR 的变量引用使用完整 `HirId` 生成唯一键，因此同名变量、遮蔽和量词绑定器有不同身份。
2. 使用 `hir_enclosing_body_owner` 找到实际所属的函数或闭包。嵌套闭包中的 `HirId.owner` 本身不足以确定 MIR body。
3. 在 `mir_built` 中按绑定 span 找到唯一的 `var_debug_info`，将其名字替换为该身份键；匹配缺失或不唯一时直接报错。
4. Charon 保留这个名字。后端通过名字将逻辑变量连接到重编号后的 LLBC local。
5. 量词和逻辑 `let` 的绑定由逻辑项的词法作用域处理，不依赖程序 local。

这是一种可跑通的原型接口。正式接口适合把 source-variable → program-place 映射放进显式元数据，取代借用 debug name 的传输方式。当前编号、span 和交换文件都只服务于同一次编译，不作为增量缓存键。

擦除使用既有宏插入的 `entry()` 分隔调用定位捕获构造块，移除捕获赋值，将规格调用换成只接受整数的 anchor。规格闭包体替换为不可达体，消除残留的借用检查和翻译需求。来自捕获的逻辑读取保留为 rustc `FakeRead`，让移动/未初始化检查继续发生，同时不建立闭包借用；这些检查语句随后被擦除。

## 原型刻意限定的范围

支持的逻辑类型是 `bool`、`i32`、无泛型且无析构的结构体，以及这些类型的共享引用。支持字面量、比较、布尔运算、`if`、局部 `let`、字段读取、纯调用、量词和蕴含。

以下内容直接报错：递归纯调用、跨 crate 纯函数、带类型参数的纯调用、带契约的纯函数、带规格或 `#[pure]` 的 unsafe 函数、类型不变式，以及尚未接入的 `old`。驱动也拒绝在原型模式下启用单态化或跳过 borrowck。算术运算暂时报错：溢出和调用定义域需要明确的验证条件，原型不借此决定最终语言语义。

程序侧的结构体在原型模式下编码为不可变记录值，赋值只支持整个 local，拒绝字段和解引用写入。原有后端把所有结构体字段标为可变，在共享引用经 MIR 临时变量传递时会触发 Why3 的区域/别名限制；这次的手写相等例子实际遇到了这个问题。正式支持可变内存需要独立处理程序值、逻辑快照与 heap 的关系。

纯调用采用展开，逻辑项大小会随调用点重复增长；正式实现可以改成共享逻辑函数定义。擦除后的闭包参数目前可能产生 `unused_variables` 警告。两点均属于这个实现版本的工程边界。

## 对后续设计的建议

THIR 提取和可执行 `#[pure]` 可以一起实现，用户体验不要求规格一定从 MIR 提取。原型中真正增加的接口工作集中在：程序位置、变量身份、规格擦除，以及程序实现和逻辑定义的连接。

建议把这些接口作为下一步设计的中心：保留现在的表层 Rust 和纯函数体验，先确定显式的变量/位置桥接与逻辑调用定义域，再决定是否将现有 MIR 规格路径整体迁移。Prusti 式 MIR 路线仍可以采用相同的纯性和快照语义；这次实现为选择提取层提供了具体的代码结构和限制。

`old` 应当围绕状态索引和快照读取单独设计。此次没有改动它的表层签名或补一个伪实现，负责 `old` 的工作可以继续独立推进。
