# 规格提取与擦除方案讨论

结论：**spec 提取 + 擦除是必须要做的。且目前的ULLBC上的collect-specs时机过晚**。

什么时候提取取决于是否希望保留 `THIR -> mir_built`，以及MIR上的 `mir_built -> mir_promoted -> mir_borrowck -> mir_drops_elaborated_and_const_checked -> optimized_mir` 的变换。

spec 可以在上述任意阶段提取，可以是THIR，也可以是早期MIR或者早期MIR生成的ULLBC；
但为了使 spec 免受“无法move out”的报错(E0382)，必须在 `mir_borrowck` 之前。

## 1. 要决定的不是一个开关

以下问题相互关联，但不必采用同一个阶段解决：

1. 怎么生成一个可以跑的代码？
2. 解除前端的运行时闭包约束。前端改成 FnOnce。FnOnce 在这里是类型检查的载体，不是规格只能使用一次的语义。
3. 解除 MIR 的线性使用约束。解除规格内部的 move/borrow 限制：提取原公式，给 borrowck

- 规格以什么形式交给 rustc 检查？闭包或再做一个 lambda lift 的整理 pass。
- 从哪里获取规格公式？THIR或早期MIR，borrowck之前。
- 公式保存在什么 IR 中？独立 Spec IR。**目前的 ULLBC body 过于离谱了**
- 何时清理辅助借用、move、调用和 drop？需要讨论
- 如何关联程序位置和变量？显式 anchor，和 lambda lift 结合。

## 2. 现有方案

### 2.1 数据流

```text
用户规格
  → 宏展开为 Rust helper 调用和闭包
  → rustc 名字解析、类型检查、闭包捕获分析
  → MIR
  → Charon ULLBC
  → collect_specs：识别 helper，拆出闭包及 captures
  → 规格 body 继续转换为 LLBC
  → WhyML Term
```

宏在函数体中插入 `entry()` 及 `precondition(...)`、`postcondition(...)` 等调用。前置条件编码成零参数闭包，后置条件闭包额外接收返回值。类型不变量采用生成 `TypeInvariant` 实现的方式。详见[宏实现](../trust2-contract-macros/src/impl.rs)和[helper 定义](../trust2-contract/src/internal.rs)。

helper 仍保留 `Fn` 及参数、返回类型约束。去掉全部调用 trait bound 则还涉及类型推导和提取器定位方法。

Vender 进来的 Charon 增加了 [`collect_specs`](../charon/charon/src/transform/collect_specs/unstructured_body.rs)，从规格调用中取得闭包，定位 `Fn` 方法，拆开闭包环境和 tuple 参数，重写 local 与投影，并构造 `SpecClosure`。断言、假设和量词被转换成特殊 IR 节点；前后置条件附着在函数声明上。类型不变量由 [`collect_specs/non_body.rs`](../charon/charon/src/transform/collect_specs/non_body.rs) 单独收集。

[`SpecClosure`](../charon/charon/src/ast/gast.rs) 保存 `Body` 和 `IndexMap<LocalId, Rvalue>` 形式的 captures。后端 [`ctx/spec.rs`](../cargo-verify/src/llbc_to_whyml/ctx/spec.rs) 将 captures 绑定到逻辑环境，再把 structured body 转换为 WhyML 项。因此规格不是只借道 MIR 定位，公式体也完整依赖程序 IR。

### 2.2 现有设计的合理性 & 主要负担

1. **规格表示混用运行时 ULLBC 表示。**
2. **提取时机太晚了，borrowck 已经跑过了。** 必须在borrowck之前介入，提取并删除闭包。

保留 MIR 获取的一个貌似理由是：**如果普通规格表达式采用受限 Rust 计算的语义，MIR 能提供明确的操作、求值顺序和控制流，验证器可通过纯编码或符号执行得到逻辑表达式。** 但：
- 很多清理工作其实在 MIR 早期就完成了，比如 name resolution, trait resolution, etc.
- 结构化控制流 -> CFG -> 结构化 Spec IR 的无意义往返
- borrowck 跑过了，直接暴毙。

borrowck 为什么不应当留：
- 因为 spec 表达的是逻辑值。存在大量方式，比如量词允许引入多个逻辑值，并用逻辑相等关联它们，见[prusti#1626](materials/prusti-issue-1626.md)。这只会让spec出现不必要的用户体验和workaround。

## 3. 已有实现

这里区分两种说法：有实现先例、具有形式化正确性证明。本文调研主要支持第一种。

### 3.1 Prusti：MIR 规格编码

Prusti 的 2022 年论文描述了从规格及纯函数的 MIR CFG 生成 Viper 表达式的 pure encoding，其中使用反向符号执行恢复结果表达式。这提供了“规格作为受限计算来翻译”的正面先例。[论文，第 4 节](https://pm.inf.ethz.ch/publications/AstrauskasBilyFialaGrannanMathejaMuellerPoliSummers22.pdf)

`prusti/prusti` 的 `425686d5ddf9d3ae3b0cec4513e899bd2eb5a663`：`MirPureEnc` 显式识别 `Forall`、`Exists`，提取闭包的 binder 与 body 后构造 VIR 量词。[`mir_pure.rs`](https://github.com/prusti/prusti/blob/425686d5ddf9d3ae3b0cec4513e899bd2eb5a663/prusti-encoder/src/encoders/mir_pure.rs)

同一版本的 `encode_spec_block` 把 pure encoder 的输入 locals 按当前 PCG 状态转换成 snapshots，再实例化公式。这提示我们：无论公式从哪一层获取，接入程序状态都应是显式步骤。[`mir_impure.rs`](https://github.com/prusti/prusti/blob/425686d5ddf9d3ae3b0cec4513e899bd2eb5a663/prusti-encoder/src/encoders/mir_impure.rs)

这一证据支持 MIR 方案的可实现性。

### 3.2 Creusot：THIR 公式与 MIR 程序分开处理

检查版本为 `616cfccc7be6d17467e5ebee6d780353b9eab374`。其 Pearlite 公式从 THIR 翻译；`mir_built` wrapper 调用原始 provider 后清理规格编码。纯规格 body 被替换，外围规格闭包的 captures 被清空，但闭包构造仍可用于识别规格及其位置。[THIR 翻译](https://github.com/creusot-rs/creusot/blob/616cfccc7be6d17467e5ebee6d780353b9eab374/creusot/src/translation/pearlite/from_thir.rs)、[查询接入](https://github.com/creusot-rs/creusot/blob/616cfccc7be6d17467e5ebee6d780353b9eab374/creusot/src/callbacks.rs)、[MIR 清理](https://github.com/creusot-rs/creusot/blob/616cfccc7be6d17467e5ebee6d780353b9eab374/creusot/src/cleanup_spec_closures.rs)

规格闭包的 `DefId` 承担身份作用，语句提供 location 和 scope。变量映射通过 `ScopeTree` 使用 `var_debug_info`、`SourceScope`、源变量标识及 shadowing 规则恢复；分析时再查询该 location 的状态。[作用域映射](https://github.com/creusot-rs/creusot/blob/616cfccc7be6d17467e5ebee6d780353b9eab374/creusot/src/translation/fmir.rs)、[变量替换和分析](https://github.com/creusot-rs/creusot/blob/616cfccc7be6d17467e5ebee6d780353b9eab374/creusot/src/analysis.rs)

这支持“THIR 提取 + built MIR 清理”的整体可行性。[Pearlite 文档](https://guide.creusot.rs/pearlite.html)

### 3.3 Verus：THIR only

检查的上游版本为 `2f9ceed94ce50054c42dc4035115b92818765bf5`。本地 checkout 是其后的 fork 合并提交，本次引用的擦除与 THIR 构建文件已确认与该上游版本一致。该版本的 verifier 将 HIR/THIR 信息转换到 VIR，并依据模式检查结果生成擦除信息；它依赖自带的 `rustc_mir_build`，在 THIR 构建路径调用 `erase_body`、`erase_tree`。[擦除信息](https://github.com/verus-lang/verus/blob/2f9ceed94ce50054c42dc4035115b92818765bf5/source/rust_verify/src/erase.rs)、[THIR 构建](https://github.com/verus-lang/verus/blob/2f9ceed94ce50054c42dc4035115b92818765bf5/source/rustc_mir_build/src/thir/cx/mod.rs)

Verus 还展示了一个独立的语言选择：其规格算术可扩大结果类型以避免溢出，并与可执行 Rust 算术区分。[规格算术](https://verus-lang.github.io/verus/guide/spec-arithmetic.html)

### 3.4 hax：THIR 可以作为保留高层结构的前端输入

hax 的架构文档描述了导出 rustc 表示、将 THIR 导入自有 AST、再执行转换的路线。这支持“THIR 到独立 IR”的工程可行性，但不是“THIR 规格 + MIR/PCG 程序关联”的直接验证。当前文档还明确区分了使用 Charon/Aeneas 的 Lean 路线与所描述的其他 backend 路线，不能把所有 hax backend 概括成同一 THIR pipeline。[架构文档](https://hax.cryspen.com/dev/architecture/)

## 4. 新方案的公共部分

### 4.1 独立 Spec IR

建议把公式表示为带类型和源码位置的逻辑表达式，而不是 `Body`。以下只是结构草图，不是已经决定的 Rust API：

```text
SpecExpr =
    Literal | BoundVar | ProgramVar
  | Unary | Binary | If | Let | Match
  | Field | Deref | Call
  | Forall | Exists | Implies
  | AtState

SpecDecl = {
    owner,
    kind,
    binders,
    expression,
    span,
    generic_environment,
}
```

`BoundVar` 表示量词、let 等引入的逻辑变量；`ProgramVar` 引用待绑定的程序变量身份；`AtState` 表示在明确状态下解释子表达式。函数返回值可以是显式 binder，不能仅凭某个生成 local 的序号识别。

普通类型、声明身份、泛型与 trait 选择尽量复用现有翻译设施；表达式不再复用程序 `Body`。原始 `DefId`、`HirId` 可用于本次编译中的关联，但跨进程、跨 crate 导出应使用有版本约束的自有 ID 和身份映射。

纯公式和以后可能需要的资源断言应保持边界。不能因为纯公式是布尔值，就把具有资源消耗语义的断言或有所有权含义的 ghost 代码全部作为普通 `bool` 处理、无条件擦除。

另一种备选是从 THIR 直接生成 WhyML AST。它省去一层自有表达式，但会让前端提取更早依赖 WhyML 的类型、绑定和状态编码。考虑到还需接入 PCG、保存规格元数据，建议优先使用小型 Spec IR，而不是把 rustc 的整套 THIR 原样复制下来。

### 4.2 规格身份、程序位置与变量绑定

```text
SpecId                        → 某条公式
SpecSiteId                    → 公式在程序中的一次静态出现
SpecSiteId + 当前 IR           → 程序点和 SourceScope
公式自由变量 + 绑定环境        → 程序 Place
程序 Place + 程序点/状态       → 该位置的逻辑值和可观察性条件
```

初版中 `SpecId` 与 `SpecSiteId` 可以一一对应，但概念上不能把它们等同于 `Local` 或 `Location`。同一个静态位置在循环中可能多次执行；一个规格也可能被实例化到多个调用点。

`SpecAnchor(id)` 是拟议的概念性标记，不是现有 rustc MIR 指令。可实现为保留无捕获的特殊闭包构造，也可保留专用 helper 标记。后者若表现为 MIR call，会带来 terminator、unwind 等处理问题，需要明确支持，不能默认它天然无影响。初版更适合复用可识别的闭包标记，在进入 Charon 时将其转换为验证元数据或显式规格位置节点。

变量关联有三种主要备选：

| 方式 | 优点 | 需要验证的部分 |
| --- | --- | --- |
| 从原始 MIR captures 提取绑定表，再清理捕获 | 利用编译器已有结构，不依赖按名字恢复 | 捕获字段到源变量的对应、部分捕获、嵌套闭包、临时借用链；不能在绑定表中引用已删除的捕获临时 local |
| 类似 Creusot，使用 `SourceScope + var_debug_info` | 原规格与程序可以分别提取，不依赖实际保留 captures | 宏 hygiene、shadowing、解构、外层闭包捕获、当前 MIR 阶段的元数据完整性 |
| 在 THIR→MIR lowering 时直接记录源变量和程序点映射 | 对应关系显式，减少恢复工作 | 编译器接入或维护补丁；后续变换仍必须维护映射 |

这里尚不承诺哪种覆盖最完整。建议先比较前两种；缺失映射必须产生明确诊断，不能静默按同名变量替代。

位置可以随一个显式节点经过 ULLBC/LLBC，也可以存入独立关联表。独立关联表并不会免除维护责任：删除语句、合并 block、重排 locals 和结构化控制流都要更新它。对于 PCG 生成的状态，必须说明对应 statement 的 before/after 还是某条控制流边，不能只保存一个模糊的“附近位置”。

### 4.3 与 PCG、后端和跨 crate 的接口

建议让程序状态解释层负责把已绑定的公式变量转换成逻辑值。纯公式提取器不直接决定 PCG 的权限规则，PCG 也不负责重新解析规格语法。

即使公式不产生运行时借用，也必须检查它是否在合法状态中观察值；“没有借用副作用”不意味着可以读取未初始化、已移动或不允许通过该路径观察的数据。

公式仅使用的程序变量不能在绑定前无意丢失。需要定义相关 local 的保留与重编号规则，或在程序简化前把规格绑定到可持续维护的逻辑值身份。

跨 crate 时不能假定能够取得依赖的 THIR。应按 crate 导出规格声明、泛型环境、必要的逻辑函数信息和稳定关联，导入时检查版本。可先将 Spec IR 放在现有导出容器的独立字段，也可使用 sidecar；两种方式都不要求把公式伪装成 LLBC body。物理上存入同一文件与语义上属于同一 IR 是两回事。

## 5. 可选路线

### A. 保留 MIR/ULLBC/LLBC 公式路线，改动最小，明确对MIR的扩展语义

Charon 仍可复用：

目前我们的 ULLBC-PCG 输入生成脚本显式指定 --mir promoted --export-borrowck，拿 borrowck 对应的 MIR body。

改成：对spec部分 --mir promoted，对程序继续 --mir promoted --export-borrowck。保存原始规格 MIR 供逻辑翻译，同时把交给 borrowck 的程序 MIR 和规格 owner body 清理掉。

一个中间变体 A′ 是 **MIR → Spec IR**：保留从 CFG 和 captures 提取公式的方式，但让公式尽早离开 ULLBC/LLBC。它可以减少后端对程序 body 的依赖，而不引入 THIR 自由变量到程序变量的全新关联问题；仍需要 pure CFG 到表达式的编码器。

### B. THIR → Spec IR，在 `mir_built` 清理，Creusot-like

```text
                   ┌─ THIR → Spec IR ──────────────────────┐
宏展开、类型检查 ──┤                                        │
                   └─ built MIR → 清理规格 → 程序 MIR        │
                                      ↓                     │
                             borrowck / PCG / Charon         │
                                      ↓                     │
                              程序 IR + 位置关联 ────────────┤
                                                            ↓
                                                       WhyML / VC
```

实施细节：

1. 用属性或解析后的 helper 身份发现 spec owners。覆盖前后置条件、内联断言、量词子闭包和类型不变量；不能仅从普通程序的可达调用集合推断。
2. 在 THIR 被消费前，将规格翻译为 Spec IR 并进行规格合法性检查。可主动查询已知 owners，也可在查询 provider 包装层提取；须避免查询环。不要假定 `after_analysis` 仍有原始 THIR。
3. 包装 `mir_built`，调用原 provider 后，在结果暴露给下游前清理规格捕获、helper 调用及相关临时操作。需要的绑定信息先保存；程序自身的计算必须保留。
4. 保留无捕获的位置标记。纯规格 owners 的原始 body 不再被 Charon 当程序翻译；编译器仍需处理它们时使用经过验证的替代 body 策略。
5. borrow facts、PCG 与 Charon 必须使用相容的清理后 MIR 及位置映射。不能让一方拿清理前 body、另一方拿清理后 body。
6. WhyML 后端消费 Spec IR 和程序状态解释，移除对 `SpecClosure.body.as_structured()` 的要求。

这条路线允许原规格短暂生成 MIR；它保证的是供后续程序分析使用的 MIR 已经清理，且公式不再从 MIR/LLBC 翻译。Creusot 是最接近的实现先例，但本项目的导出、PCG 和 CFG 结构化仍需适配。

### C. THIR 提取并提前擦除，不构建原规格公式的 MIR，相比 B 更干净，但 stock rustc 的行为还不知道

从原始 THIR 提取公式，再给 MIR builder 提供擦除后的输入。父函数中的规格闭包构造和调用整体替换成标记或 unit；纯规格闭包及类型不变量方法也要处理。

可包装 `thir_body`，也可像核对的 Verus 版本一样，在 THIR 构建中加入专门支持。二者不能视为相同的工程成熟度。stock rustc 查询包装的完整实现还需要原型验证。

THIR 不仅供 MIR builder 使用，也供 match、unsafe 等检查使用。提前擦除会改变这些检查看到的内容，必须明确原规格通过哪些检查；不能把被跳过的检查默认为已通过。[rustc THIR 文档](https://rustc-dev-guide.rust-lang.org/thir.html)

还需区分两个目标：

- **原规格公式不生成 MIR。** spec owner 被请求 MIR 时，可直接提供满足编译器要求的占位 body，不调用原始公式的 MIR builder。
- **spec owner 连占位 MIR 都没有。** 必须确保所有分析、body 枚举、metadata 和 codegen 路径都不会请求它的 MIR。过滤普通可达调用或单独过滤 `mir_keys` 不足以完成这一保证。

前者是可研究的优化和隔离目标；后者涉及更大范围的编译器流程控制。仅“删掉父函数中的闭包表达式”不等于删除了闭包 item。

此外，THIR 位于类型检查之后。即使不构建规格 MIR，闭包 kind、外层闭包捕获以及 trait 约束也可能已经受规格影响。提前擦除不自动撤销这些前端结果。

### D. 更独立的规格前端与程序编译

把规格检查、逻辑 IR 构建与可执行程序编译进一步分开。

这可以更直接地定义逻辑类型、数学运算和 ghost/exec 边界，但也需要承担类型环境对应、模式检查、擦除一致性和诊断来源的设计。它不只是换一个提取 API。本次不建议以此为实施目标；Verus 的相关设计适合作为长期语言设计参考，不能直接套用到当前 Charon pipeline。

## 6. 对语言设计的影响

### 6.1 语义选择与提取阶段正交

如果规定规格表达式按 Rust 规则求值、正常终止并返回 `true`，则必须处理其溢出、panic、调用和终止行为。在这一前提下，MIR 是自然的输入；但使用 THIR 也可以保留同样的语义。

如果规格包含数学量词、多状态观察或其他专门逻辑构造，则它已经是扩展语言。MIR 可把这些构造作为 intrinsic 保存，Spec IR 可直接表示它们。两者都需要正式或至少足够精确的语义说明。

因此以下推论均不成立：

- 有 `forall/exists`，所以不能用 MIR。
- 用 THIR/Spec IR，就必须把机器整数改成数学整数。
- 用 MIR，就自动继承了全部正确的 Rust 语义。
- 取消运行时借用，就可以在规格中无条件观察所有程序值。

### 6.2 本次改造应保留和应明确的内容

第一阶段尽量保留既有语法和已有可确认的语义，不将语言变更混入架构迁移。当前行为若没有清晰定义或存在缺陷，应显式记录，不能将旧实现的所有输出视作语义标准。

至少要逐项明确：

| 项目 | 必须回答的问题 |
| --- | --- |
| 算术与转换 | 机器整数还是数学整数？溢出、除零、越界和 cast 如何解释？ |
| 量词 | 量化域是什么？Rust 类型有效性与类型不变量是否限制该域？binder 和 trigger 如何表示？ |
| 逻辑调用 | 哪些函数可调用？如何表达纯性、终止性、前置条件及未解释函数？ |
| `old` 和返回值 | 切换的是哪个状态？`old(*p)` 是否保存入口时读取的值，而非只保存引用？ |
| 程序观察 | 对已移动、部分初始化或借出的 place，允许怎样的逻辑观察？ |
| 规格块 | 是否允许局部 let、match、局部可消除赋值？是否支持循环？ |
| ghost 与资源 | 哪些是纯公式，哪些操作具有证明状态或资源效果？ |

Spec IR 的主要语言设计优势是把这些区别表达在结构中，而不要求先从程序 helper、控制流和引用操作中恢复。它不免除语义定义，也不自动扩大 rustc 类型检查能够接受的表达式集合。

## 7. 优劣比较

| 维度 | A / A′：MIR 公式 | B：THIR 公式 + built MIR 清理 | C：THIR 提前擦除 |
| --- | --- | --- | --- |
| 已有先例 | 本项目已有部分链路；Prusti 有 MIR pure encoder | Creusot 有非常接近的分工 | Verus 有专门支持；本项目查询包装未验证 |
| 语言表示 | 普通计算自然；逻辑扩展需保持 intrinsic | 逻辑构造直接进入 Spec IR | 与 B 相同；提前擦除本身不增加表达能力 |
| 变量关联 | 可利用 MIR captures 和 locals | 需源变量到程序 place 的绑定层 | 仍需绑定层，且可能失去原始 captures 这条信息来源 |
| 对程序分析的隔离 | 原样晚清理有风险；可另加早期分流 | 在分析前集中清理 | builder 输入中已无原规格表达式 |
| 代码清晰度 | 统一程序表示，但规格需经过更多程序 pass；A′ 可缓解 | 公式与程序职责清楚，新增显式交界层 | 公式层同 B；编译器检查及查询管理更复杂 |
| 实施工作 | 保留 A 较少；补齐语义审计仍有工作；A′ 中等 | 新提取器、IR、绑定层、导出与后端迁移，工作量中等偏大 | B 的工作之外，还需 THIR 擦除和检查完整性处理 |
| 维护重点 | MIR/闭包形状、pure encoder、程序 pass 的规格不变量 | THIR 覆盖、状态绑定、查询时机 | B 的维护加 THIR 构建、擦除与编译器更新适配 |

### 7.1 性能：目前只能给出机制分析

本次没有运行性能对比，不能给出“THIR 一定更快”的结论。

- B 可以省掉规格的 Charon 翻译、LLBC 控制流重构及部分后续 pass，但仍支付最初的 MIR 构建成本，并新增 Spec IR 提取和变量绑定成本。
- C 可能进一步减少公式 MIR 的构建与分配；是否值得取决于规格规模、查询次数及擦除开销。
- 为方便提取而长期保留全部 THIR，可能增加峰值内存。应优先转换成自有 IR 后释放不再需要的 rustc 表示，而不是永久保存完整 THIR。
- A/A′ 可以复用统一的声明、类型和 CFG 编码；其缓存收益不能仅凭 IR 层数判断。
- 验证总耗时还受生成公式形状、共享方式、量词与求解器影响。相同语义的不同编码也可能有明显不同的求解成本。

若需要比较，应固定语义、编译配置和用例，分别记录提取/翻译时间、峰值内存、生成 IR/公式规模及 Why3/solver 时间。只比较总时间难以定位收益来源。

## 9. 尚未决定的问题

以下项目需要在实现前或最小原型阶段确定：

1. 变量映射优先使用原始 captures、作用域元数据，还是二者有明确分工？
2. 标记以何种形式进入 Charon；在 ULLBC/LLBC 中保留节点还是独立位置表？
3. Spec IR 存入现有导出容器还是单独文件，版本如何同步？
4. 当前普通规格表达式的运算、panic 和终止语义有哪些已经定义，哪些只是实现行为？
5. 普通闭包内部出现规格时，类型检查阶段的捕获影响如何处理？
6. 规格仅使用的 locals 如何在程序清理与重编号后仍可绑定？
7. 如何安排 THIR 提取、MIR 查询及缓存生命周期，避免 query cycle、stolen body 和增量构建漏掉提取副作用？
8. 未来的资源断言与 ghost 代码是否采用独立表示，哪些状态规则由 PCG 解释层负责？
