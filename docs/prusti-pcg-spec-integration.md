# 从 ULLBC-PCG 到规约框架：以 Prusti 的接入方式为参照

调研日期：2026-10-24。目标是解释“已有 PCG 算法之后，还要实现什么”，而不是再次介绍 PCG 算法。

源码基线：工作区 `prusti/` 的 `c18d4bdc327ac0788a380dfd220ee82b416a022e`，其 PCG 为 `cd7bcbdb1a429fedaf918a7efd4a0be5d49d2384`；本项目 `repo/` 为 `4ceaabb`。调研时上游 main 为 `73bb6908f2d25736bd085489d6aebdc1b36faff7`，比较结果显示本文重点分析的 MethodEnc、MirSpecEnc、mir_impure、fn_wand、loop 和 indirect 编码文件未改变。本文以固定基线为准。

这是源码级调研，没有运行 Prusti 或 Why3。文中的简化公式、验证代码和建议接口是解释或设计草案，不是运行产生的输出。当前目录没有可直接使用的 Prusti 构建产物。

## 1. 先确定要接通哪两条路

你的目标有两部分：

1. **隐式资源规约**：用户不写，工具根据 ULLBC-PCG、类型和签名生成资源条件及资源操作。
2. **显式用户规约**：用户可以写功能性质，也可以写权限、points-to 等资源性质；这些表达式要使用同一个资源与值模型进行验证。

Prusti 提供的可直接参考部分是：**自动资源编码与用户功能规约，最终在验证 IR 中汇合。** 用户的借用到期规约（pledge）还会嵌入自动生成的资源归还契约。

但在检查的用户接口中，没有找到供用户直接书写通用 `acc`、points-to、分离合取和 wand 的完整资源规约语言。`predicate!` 是纯逻辑谓词的语法，不能等同于内部的 Viper 权限谓词。这部分需要你们额外设计。[S7][S8]

```text
类型、签名 ─────────────→ 自动函数资源契约 ───────────┐
ULLBC-PCG 状态/动作 ─────→ 资源解释与证明操作 ─────────┤
用户功能规约 ───────────→ 逻辑表达式、状态读取 ────────┤→ 验证 IR → 后端证明
用户显式资源规约 ───────→ 资源表达式及其检查/转移 ─────┘
                           ↑ 你们要补充的用户接口
```

“把 PCG 写成规约”至少包含三种产物：函数前后置条件、语句间的证明操作、循环资源不变式。不能只在函数上增加两个权限字段。

## 2. 最先读 MethodEnc：隐式与显式在这里合并

入口是 `prusti-encoder/src/encoders/mir_fn/method.rs`。[S1]

`MethodEnc` 分别取得：

- `MirSpecEnc` 的结果：用户的前置条件、后置条件、pledges。
- `WandEnc` 的结果：根据函数签名的借用关系建立的间接资源和资源归还契约。
- 参数和返回值的类型编码：直接资源谓词。

源码的核心结构可缩写为：

```rust
pres.push(argument_direct_resource);       // 对每个输入参数
posts.push(result_direct_resource);
pres.extend(wands.indirect_pres(...));
posts.extend(wands.indirect_posts(...));
posts.extend(wands.wand_posts(...));

pres.extend(spec.pre_exprs());              // 用户 requires
posts.extend(spec.post_exprs());            // 用户 ensures

vcx.mk_method(..., pres, posts, body)
```

由此可以得到三个明确结论：

1. **自动条件不经过用户规约宏，也不先生成 Rust 规约闭包。** Prusti 的 encoder 直接构造验证 IR 表达式。
2. **不是所有隐式资源条件都来自函数体 PCG。** 函数入口/出口的资源接口主要来自参数类型、返回类型，以及 PCG 库对函数签名建立的 `FunctionShape`。没有可见函数体的调用也需要有资源接口。
3. **用户前后置条件与自动资源条件进入同一个 method contract。** 用户表达式可以在自动资源条件提供的权限下读取数据。

例如下面只是说明合并关系：

```rust
#[requires(*p >= 0)]
#[ensures(*p == 1)]
fn set(p: &mut i32) {
    *p = 1;
}
```

省略引用自身、unit 返回值等资源，逻辑上可以读作：

```text
入口：拥有 p 指向的 i32 的资源，而且其值 >= 0
出口：归还该 i32 的资源，而且其值 == 1
```

其中“拥有/归还资源”来自自动编码，“>= 0 / == 1”来自用户。PCG 不负责证明数值等式；Prusti 的赋值和值模型负责连接程序执行与该等式。

**对本项目的对应点**：`cargo-verify/src/llbc_to_whyml/ctx/decl.rs` 目前从 `FunSpecs` 翻译用户 pre/post。将来应在这一层组合生成的资源条件与用户条件。`FunSpecs` 目前只有 `Vec<SpecClosure>`，不意味着自动资源也必须伪装成 `SpecClosure`。

## 3. PCG 与 Prusti 的职责，按输入输出分开

| 工作 | PCG 提供什么 | Prusti 额外实现什么 |
|---|---|---|
| 当前资源状态 | place、capability、借用依赖、历史节点等 | 这些对象在验证语言中表示什么资源 |
| 单条语句 | 四阶段状态与有序动作 | 将选定动作变成证明操作，并编码语句的实际值变化 |
| 读写位置 | 哪些 place 可读写、如何获得对应能力 | place → 地址；从地址读值；赋值修改哪个逻辑位置 |
| 函数边界 | 签名中的借用依赖/FunctionShape | 自动 requires/ensures、归还契约、调用和返回的证明代码 |
| 循环 | 循环处状态、摘要边、相关使用信息 | 资源不变式；与用户功能不变式合并 |
| 用户断言 | 不理解用户断言的功能含义 | 解析、提取、编码断言；选择状态和读取方式 |
| 借用结束时的用户性质 | 提供借用依赖解除的时机和步骤 | 将 pledge 放进归还契约并在到期处检查/应用 |
| 最终证明 | 产生分析结果 | Prusti 生成验证程序，由 Viper 验证 |

PCG 可以计算 require/ensure 条件并检查其图操作，不能因此把它当成用户规约的定理证明器。本项目 `pcg/triple.rs` 中的 `Triple.pre/post` 也是 **PCG 分析的转移条件**，不是已经接入 Why3 的函数契约。

## 4. Prusti 必须先给资源和数据值一个解释

PCG 里的 `Place(x)`、`x: E` 本身不告诉 Why3/Viper 怎样存储整数，也不定义某个逻辑谓词。

Prusti 的类型编码器为类型建立权限谓词和取值函数。用示意记号描述：

```text
P_T(a)       地址 a 处的 T 类型资源
snap_T(a)    该位置当前值的逻辑表示
```

`PredicateBuilder::mk_snap_function` 为 snapshot 函数加入相应权限前置条件。因此“用户表达式能读取一个值”与“自动资源编码提供了读取所需的权限”能在同一个逻辑模型中连接起来。[S2]

例如 `&mut i32` 有两个需要区分的对象：

```text
变量 r 里面存着的引用值       → 引用自身的资源/地址信息
r 指向的 i32                → 目标位置的 i32 资源和当前值
```

当前 Prusti 的可变引用 snapshot 是浅的。函数规约或语句规约中解引用可变引用时，`MirPureEnc::encode_place_element` 先取目标地址，再调用 referent 的 `ref_to_snap` 读取当前值。它没有把引用 snapshot 里的任意 value 字段当作目标当前值。[S3]

`IndirectPredicatesEnc` 则把 lifetime projection 解释为间接资源。例如对 `Holder<'a> { r: &'a mut i32 }`，检查字段和 lifetime，得到该引用目标的 `i32` 资源谓词。这是 **Prusti 对 PCG 节点的解释**；该节点和依赖关系由 PCG 提供。[S4]

**对本项目的对应点**：目前 `ctx/place.rs` 的 `Deref` 返回 base，`ctx/rvalue.rs` 的可变借用仍为 `todo!()`。因此即使把所有 PCG 状态存进 LLBC，后端仍然不知道 `*r = 1` 改的是哪个位置。需要先建立 place、引用目标、当前值和资源之间的一致关系。

## 5. 函数体如何把 PCG 结果用起来

### 5.1 不把每个 capability 变化机械地变成断言

主要入口是 `mir_impure.rs` 的 `pcg_actions`、`borrow_action`、`pcg_repack` 和 `visit_statement`。[S5]

语句编码顺序是：

```text
PreOperands 动作
读取并保存操作数值（move 还可能消耗源资源）
PostOperands 动作
PreMain 动作
编码真实赋值等效果
PostMain 动作
```

Prusti 对选定动作生成 unfold/fold、exhale 或 wand 操作；其他动作可能只更新分析关系，或由实际语句编码承担效果，或在当前实现中被忽略。不支持的部分 owned repack 会生成 `exhale false`。

特别是：PCG 中“源 place 因为借出而不再有 capability”，不能直接推导为“Viper 此时必须删除该地址的所有权限”。同一个地址随后还要通过借用引用访问。**PCG 的可访问路径限制与后端持有的地址资源，不是逐项同名的表。**

### 5.2 用你当前支持的样例理解两边的分工

```rust
fn example() -> i32 {
    let mut x = 0;
    let r = &mut x;
    *r = 1;
    x
}
```

下表是语义解释，不是 Prusti 输出的逐行 Viper：

| 程序动作 | 你当前 ULLBC-PCG 已经在做 | 后端编码仍需要做 |
|---|---|---|
| `x = 0` | 设置初始化/能力状态 | 建立 x 的逻辑位置及其值 0 |
| `r = &mut x` | 建立 borrow edge，调整 x/r 能力 | 表示 r 的目标就是 x 的位置；建立引用值 |
| `*r = 1` | 按需展开 deref，准备写、恢复能力 | 经 r 找到同一位置，将其值更新为 1 |
| `x` | 结束不再活跃的借用，恢复 x 的能力 | 读取那个位置；其值应当是先前写入的 1 |
| `ensures(result == 1)` | 不推导此数值性质 | 将用户后置条件与返回值、内存模型连接，产生证明义务 |

这里即使没有 struct、函数返回借用或循环，也已经能验证“PCG 被规约框架实际消费”。不需要先实现 magic wand 才开始。

### 5.3 当前实现不能被当作完整独立权限检查器直接照搬

`TyImpureEnc` 生成的抽象 `assign` 方法没有资源前置条件，后置条件给出目标资源和赋值后的 snapshot；写入前的资源处理分散在 PCG 动作及语句编码中。[S2]

这表明当前实现使用了编译器/分析结果与编码器约定，并不逐项把 PCG 的 E/R/W/e 重证为四类后端断言。论文描述的 core proof、当前具体实现、你们希望验证的显式资源性质，需要分别说明。若你们希望独立检查资源流，必须给自己的读、写、借用、归还操作定义可检查的前后条件；不能把 `SetCapability(..., E)` 直接解释为无条件获得可写资源。

## 6. 用户规约怎样与资源分析连接

### 6.1 前后置条件：用户表达式走另一条编码路径

```text
#[requires] / #[ensures]
  → 宏生成 spec-only 项并赋予 spec ID
  → 提取并关联到被标注的函数
  → SpecEnc 查询规约项
  → MirSpecEnc / MirPureEnc 编码为逻辑表达式
  → MethodEnc 与自动资源条件合并
```

宏层负责提供 Rust 类型检查和身份关联；用户表达式的含义最终由后端编码。PCG 不解析 `requires` 的表达式。[S6][S7]

### 6.2 函数体断言：当前 PCG 状态帮助准备表达式的输入

`encode_spec_block(spec_block, pcg_state, ...)` 做了一个很直接的衔接：[S5]

1. 用 `MirPureEnc` 编码用户表达式，得到它需要的输入 locals。
2. 根据当前 PCG 的展开状态，为这些 locals 构造 snapshots。
3. 若 struct 已被拆开，递归从字段 snapshots 组装表达式输入；被移出/仅有 W 的部分可能用未约束 snapshot 表示，部分缺失能力会报错。
4. 将这些输入代入用户逻辑表达式。
5. 将表达式发给验证器检查。

当前 `prusti_assert!` 的编码使用 `exhale` 检查纯布尔表达式，`prusti_assume!` 使用 `inhale`。对纯布尔表达式，exhale 不消耗堆权限；但如果你们以后允许资源断言，不能自动沿用相同语义，否则“断言拥有资源”可能变成“消耗资源”。

本项目对应的是 `translate_spec_closure`：目前只有捕获变量绑定和闭包体翻译。将来需要明确其观察的程序点/状态，并将引用读取接到统一的值模型。资源检查还要防止规约通过被阻塞的别名读取数据。

### 6.3 循环：资源不变式与功能不变式汇合

`get_loop_inv` 从循环边界的 PCG 状态和 place usages 得到资源不变式，包含位置资源、间接资源和相应 wands。`visit_basic_block_data` 再将其与用户循环不变式合并。[S9]

它不是自动推导排序、计数等功能不变式。对本项目，这是后续扩展；当前 ULLBC-PCG 明确拒绝循环，不能先宣称已能生成循环规约。

### 6.4 借用到期规约：两层结合最明显的例子

Prusti 仓库里有如下用户规约样例：[S10]

```rust
#[assert_on_expiry(*result == 42, true)]
fn must_answer(location: &mut i32) -> &mut i32 {
    location
}
```

用户要求：返回的借用结束时，调用者必须使目标值为 42。

分工如下：

| 环节 | 谁做 |
|---|---|
| 知道返回引用依赖输入引用 | PCG 库的函数签名分析 |
| 知道调用处何时解除这一依赖 | 函数体 PCG |
| 写出 `*result == 42` | 用户 |
| 把依赖编码为资源归还 wand | Prusti 的 WandEnc |
| 把用户条件编码进 wand | MirSpecEnc + WandEnc |
| 生成 package/apply 操作 | Prusti，根据 PCG/UnblockGraph 的结果 |
| 检查到期时条件是否成立 | Viper |

一般形式（`Res` 表示资源，`A/B` 表示用户纯条件）：

```text
(Res(返回借用) ∧ A) --* (Res(被阻塞的输入) ∧ B)
```

`--*` 是资源归还契约：交回左侧资源并满足 A，才能恢复右侧资源并获得 B。实际使用 Viper 的资源断言语义；不能将该公式照抄成普通 WhyML 布尔蕴含。

`MirSpecEnc` 把用户条件分为 `expiry_obligation` 和 `expiry_postcondition`；`WandEnc::mk_wand` 分别加入左右两侧。callee 返回前通过 `package_wands` 建立归还证明；caller 的 PCG 指示依赖解除时，Prusti 生成 `apply_wands`。[S11]

## 7. 你们额外需要实现什么

以下为本项目建议，不是 Prusti 已提供的接口。

### 7.1 让 LLBC 能消费 ULLBC-PCG 的结果

当前 `Analysis → PointResult → PhaseResult → AppliedAction` 已保存位置、阶段、动作前后图，是接入的良好起点。但它引用 ULLBC 的 block、statement、local 标识；LLBC 结构化、复制分支尾部、清理局部变量后，不能靠旧编号直接查。

第一版可以让“分析结果绑定 + 必要证明操作”在 ULLBC 结构化前进入转换流水线，并让结构化保留这些语义边界。可用节点附件或内部 ghost 操作，具体形式需要结合 AST 设计选择。

要保留：操作数读取与主效果之间的边界、动作顺序、place 对应关系、规约读取点。不要只保存“该 LLBC 语句结束后的图”。后续支持分支/循环时再加入 CFG 边动作和摘要。

Prusti 直接围绕 MIR 消费 PCG，没有替你们解决 ULLBC → LLBC 的资源信息保留问题。

### 7.2 建立可共用的内部规约/资源表示

建议在语义上区分下列对象（名称仅为草案）：

```text
PureExpr              数值、逻辑性质
ResourceAssertion     持有哪些资源、借用关系、资源组合
ResourceOp            检查、转移、拆分、归还等内部操作
GeneratedContract     自动函数资源接口
UserContract          用户功能条件和显式资源条件
```

自动生成的内容和用户编写的内容应该使用同一套底层资源含义，但保留来源信息，以便诊断。

这不一定要求马上修改现有 `FunSpecs`；也可以先用验证专用的扩展 IR。关键是不能把具备资源消耗语义的断言全部降成没有特殊含义的 Rust `bool` 闭包。

### 7.3 显式资源规约要先分清“检查”与“转移”

例如拟议语法 `writable(p)`、`points_to(p, v)`，需要回答：

| 使用场景 | 必须定义的语义 |
|---|---|
| `assert writable(p)` | 只检查当前状态，还是消耗一份资源？通常断言应只检查 |
| `requires owns(p)` | 调用者是否将资源交给 callee？ |
| `ensures owns(p)` | callee 是否把资源交还给调用者？ |
| `points_to(p, v)` | 是否同时包含权限和存储值；与其他别名是否指向同一资源？ |
| `P * Q` | 是否要求资源可分解，怎样处理重复可写权限？ |

只对 PCG 图查询 `can_write(p)`，可以作为“分析状态检查”功能；这不等于已经实现可传递、可分离的 points-to 资源契约，也不能单独证明数据值正确。

还需要规定自动资源与用户资源的合并策略。例如自动推导已经要求独占 p，用户又写一次 `owns(p)`：不能默认生成两份独占资源。第一版可让用户资源要求作为对自动推导资源的检查/约束，并明确诊断不兼容条件；以后再支持更一般的资源转移契约。

在 safe Rust 中，用户资源规约不能仅凭一个声明绕过 rustc 禁止的别名访问。原始指针或 unsafe 的显式资源验证属于进一步扩展，不能认为 Prusti 当前这条 safe 引用路径已经提供。

### 7.4 后端要补齐共享的资源与值语义

| 当前文件/模块 | 已有职责 | 接入需要补充的职责 |
|---|---|---|
| `ullbc-pcg/src/results.rs` | PCG 状态和动作输出 | 与转换中程序点/place 的稳定关联 |
| Charon ULLBC → LLBC 流水线 | 结构化与清理 | 保留/重映射资源动作和规约观察点 |
| `ctx/place.rs` | place/operand 翻译 | 引用目标、地址或等价逻辑身份、读值语义 |
| `ctx/rvalue.rs`、`ctx/stmt_expr.rs` | 表达式与语句效果 | 借用、写入、move 和资源操作的配套编码 |
| `ctx/spec.rs` | 用户规约闭包翻译 | 在指定状态读取值；资源表达式的识别/解释 |
| `ctx/decl.rs` | 用户函数 pre/post | 自动资源契约与用户契约合成 |
| `ctx/func_ref.rs` 等 | 内建操作 | `old` 及将来的到期观察语义 |

WhyML 中可选择适合受支持子集的显式 store/权限状态或其他有证明依据的表示。Prusti 的 Viper fold/package/apply 不能作为无定义的同名函数搬过去。

## 8. 从哪里开始学习和实现

建议按下面顺序读，每一步先回答一个问题：

1. **读 `MethodEnc` 的契约组装。** 问题：自动条件和用户条件在哪里汇合？先看第 198–247、375–390 行，不必先读整个 MIR visitor。
2. **读类型资源与取值函数。** 问题：为什么拥有资源就能读取规约中的值？看 `ty/impure.rs::mk_snap_function`，再看 `mir_pure.rs` 的可变引用 Deref。
3. **读 `visit_statement` 和 PCG 动作分派。** 问题：分析动作如何与真实赋值交错？先限定到你的直线标量可变借用例子。
4. **读 `encode_spec_block`。** 问题：用户断言怎样读取当前 PCG 状态下的数据？
5. **最后读 `fn_wand.rs` 和 `loop.rs`。** 等函数返回借用、循环进入你的支持范围后，再研究到期规约与摘要。

第一条实现验收线可以继续使用当前样例，并在内部挂一个后置条件：

```rust
// 拟验证的条件：result == 1
fn mutable_borrow() -> i32 {
    let mut x = 0;
    let r = &mut x;
    *r = 1;
    x
}
```

按顺序完成：

1. ULLBC-PCG 结果在 LLBC 中仍可定位到正确的语义边界。
2. 后端表示 `r` 与 `x` 对应同一目标，执行 `*r = 1` 后读 `x` 得到 1。
3. 正确后置条件可证明；改为 `result == 2` 应失败。只打印图或生成文件不算完成。
4. 增加一条显式资源检查，确认它消费的是同一个资源模型；错误资源要求应失败。
5. 再接入用户宏捕获与源码规约，并明确 ghost/spec 辅助借用的分析与擦除策略。

第 4 步可以先从内部规约 IR 构造，避免语法宏、PCG、LLBC 映射和后端同时调试。它是完整用户接口的中间验证步骤，不是最终完成。

错误资源要求的测试可以在内部 IR 层构造；不应要求 rustc 接受本来就违反 safe Rust 借用规则的源程序。

## 源码依据

下面链接固定到本次调研版本；工作区也有对应的 `prusti/` 源码，可直接搜索函数名。

[S1]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-encoder/src/encoders/mir_fn/method.rs#L198-L390
[S2]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-encoder/src/encoders/ty/impure.rs#L154-L353
[S3]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-encoder/src/encoders/mir_pure.rs#L1133-L1173
[S4]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-encoder/src/encoders/ty/indirect.rs
[S5]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-encoder/src/encoders/mir_impure.rs
[S6]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-encoder/src/encoders/pure/spec.rs
[S7]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-contracts/prusti-contracts-proc-macros/src/lib.rs
[S8]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/docs/user-guide/src/verify/predicate.md
[S9]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-encoder/src/encoders/impure/loop.rs
[S10]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-tests/tests/verify/pass/erdinm/pledges-basic-1.rs
[S11]: https://github.com/prusti/prusti/blob/c18d4bdc327ac0788a380dfd220ee82b416a022e/prusti-encoder/src/encoders/impure/fn_wand.rs
