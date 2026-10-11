# cargo-verify 手册（LLBC → WhyML 契约验证工具链）

本文档介绍 `cargo-verify` 的工作流程、代码文件结构，以及**当前能够翻译的 Rust 构造**与已知限制。
契约（前置/后置条件、断言、假设、量词等）由 `trust2-contract` 包写在 Rust 源码里，由本工具静态验证。

## 1. 工作流程

```
Rust 源码 + trust2-contract 契约
        │  cargo verify            （cargo-verify/src/main.rs）
        ▼
   Charon：解析为 LLBC 中间表示（.llbc）   （cargo-verify/src/rust_to_llbc.rs）
        │
        ▼
   LLBC → WhyML 翻译（.mlw）              （cargo-verify/src/llbc_to_whyml*）
        │
        ▼
   why3find 调用 Alt-Ergo / CVC5 / Z3 证明（cargo-verify/src/whyml_verify.rs）
```

产物目录由命令行参数决定：

- `--charon-out-dir <dir>`：Charon 输出的 `*.llbc`
- `--why3-out-dir <dir>`：生成的 `*.mlw`、支撑库 `trust2_contract/`、以及每次按需生成的 `trust2_contract/tuple.mlw`

两者都不指定时使用临时目录并在结束后删除。加上 `--charon-pretty-print` 可以把 LLBC 打印出来排查翻译问题。

**每次运行都会先打印一行 `Hello, TRust2-Contract`**，用于确认当前使用的是新版工具链（见 `cargo-verify/src/main.rs` 的 `main`）。

## 2. 命令行

```sh
cargo install --path ./cargo-verify
cd <待验证的 crate>
cargo verify --charon-out-dir llbc_out --why3-out-dir why3_out
```

常用参数：`-P/--prover`（默认 alt-ergo、cvc5、z3）、`--why3find-config-arg`、`--why3find-prove-arg`、`--` 之后的参数透传给 `cargo build`。

### 2.1 输出说明

```
$ cargo verify --charon-out-dir llbc_out --why3-out-dir why3_out
Hello, TRust2-Contract
...
Theory test01.Top: ✔ (3)
Proved (test01.mlw) ✔
      Proved test01.Top.max''_test01 with alt-ergo in 0.02s
      Proved test01.Top.square''_test01 with alt-ergo in 0.03s
      Proved test01.Top.strict_sub''_test01 with alt-ergo in 0.05s
    Verified test01 in 1.46s
```

- `Proved <目标> with <证明器> in <耗时>`：每个证明目标一行，目标名为 `<crate>.<theory>.<goal>`，便于回到生成的 `.mlw` 中定位
- `Stuck <目标>`：该目标未能证明（没有证明器与耗时；整体验证会失败并报 `failed to verify crate ...`）
- `Tactic <目标> (<战术>)`：该目标由战术（如 `split_vc`）拆分后证明，随后逐行打印子目标
- 数据来源：why3find 写出的 `<why3-out-dir>/<crate>/proof.json`；若使用汇总模式（自定义 `--why3find-prove-arg` 等），则回退读取 `<why3-out-dir>/why3find.proofs/<crate>.json`
- 解析使用 `utils::why3find_proof::ProofFile`，实现见 `cargo-verify/src/whyml_verify.rs`；没有生成证明记录时（例如 why3find 尚未真正开始证明）只跳过，不报错

## 3. 代码文件结构

### 3.1 顶层（`cargo-verify/src/`）

| 文件 | 作用 |
| --- | --- |
| `lib.rs` | 库入口：声明 `llbc_to_whyml`/`rust_to_llbc`/`whyml_verify` 三个模块，提供 `run_with_dir`、`run_with_temp_dir`（准备输出目录，未指定时用临时目录并在结束时清理） |
| `main.rs` | `cargo verify` 子命令的 CLI（clap 定义、参数解析、打印版本标记），按顺序驱动 "Rust→LLBC" 与 "LLBC→WhyML→证明" 两个阶段；含 8 个命令行解析单测 |
| `rust_to_llbc.rs` | 第一阶段：调用内嵌的 Charon 把当前 crate 翻译成 LLBC（自动开启 `trust2-contract/verify` feature），再把输出目录里的 `*.llbc` 反序列化成 `HashMap<crate 名, TranslatedCrate>` |
| `whyml_verify.rs` | 第三阶段：调用 `why3find config` / `why3find prove`；验证后读取 why3find 写出的 `proof.json`，逐条打印每个**证明目标、证明器与耗时**，最后打印 `Verified <crate> in <时间>` |
| `bin/charon-driver.rs` | Charon 的 rustc driver 薄封装（Charon 侧使用，与翻译逻辑无关） |

### 3.2 翻译器入口（`cargo-verify/src/llbc_to_whyml.rs`）

模块根，也是流程总览：`translate_crates` 对每个 crate 调用 `ctx::translate_crate`、写出 `<crate>.mlw`、释放内嵌的支撑库 `trust2_contract/*.mlw`，并把本次用到的元组投影汇总生成 `tuple.mlw`；`write_file` 负责格式化成 WhyML 文本。

| 子模块 | 作用 |
| --- | --- |
| `builder.rs` | `Expr`（可执行代码）与 `Term`（逻辑项）两种输出共用的构造器抽象 `Builder`，及其两个实现 `ExprBuilder`/`TermBuilder`。翻译逻辑只需针对 `T::T` 编写 |
| `error.rs` | 翻译错误 `Error` 与 `Result`（不支持联合体、原始指针、内联汇编、规格中的循环/嵌套规格等） |
| `ident.rs` | WhyML 标识符的构造：`translate_ident`，以及临时变量、基本块、元组/枚举访问函数、`break`/`continue` 异常、中缀运算符（`= < <= > >= <>`）、`return` 标签 |
| `prelude.rs` | 输出中反复使用的静态项：对所有支撑库的 `use` 声明（`IMPORTS`/`IMPORT_REF`）、`string`/空类型/单元类型、通配模式与 `true`/`false`/`()`/`absurd` 等常量 |
| `name_map/mod.rs` | 名字映射的公开类型：`NameMap`、`TypeDeclNames`、`ConstructorIdents`、`FuncDeclNames`、`LocalNames` 等 |
| `name_map/names.rs` | 名字字符串的生成：支撑库别名（`Bool'`、`I32'`…）、临时变量名、元组/枚举访问函数名、`break`/`continue` 异常名 |
| `name_map/keywords.rs` | 大小写规范化（驼峰/大写 → 小写下划线）与 WhyML 关键字规避（`TAKEN_WORDS_SET`） |
| `name_map/locals.rs` | 局部变量名生成（`LocalMapper`）与重名/关键字冲突消解（`resolve_collision`） |
| `name_map/build.rs` | 遍历 crate 收集所有条目的名字路径并构造 `NameMap` |
| `trust2_contract/*.mlw` | 内嵌的 WhyML 支撑库（整数 `i8..i128`/`u8..u128`、`bool`、`char`、`builtin`），随二进制发布并每次释放到输出目录 |
| `trust2_contract/tuple.mlw` | **运行时生成**：本次翻译用到的元组字段访问函数 `get_tuple<arity>''_<field>` |

### 3.3 翻译上下文（`cargo-verify/src/llbc_to_whyml/ctx/`）

`Ctx` 保存单个 crate 的翻译状态（输出声明、类型参数与局部变量作用域栈、循环深度）；翻译逻辑按主题拆分：

| 文件 | 作用 |
| --- | --- |
| `mod.rs` | `Ctx` 定义、作用域/声明栈操作（`with_generic_params`、`with_locals`、`get_local`、`enter_loop`、`push_decl`…）、函数应用辅助（`translate_apply*`）与 crate 级入口 `translate_crate` |
| `ty.rs` | LLBC 类型 → WhyML 类型（整数/布尔/字符/元组/引用/函数指针/空类型/类型变量） |
| `type_decl.rs` | 类型声明：`struct` → 记录，`enum` → 代数类型 + 构造子/字段访问函数（仅无字段变体），`Opaque` → 抽象类型 |
| `place.rs` | 位置表达式（局部变量、字段/元组投影、解引用、全局量）与操作数（拷贝/移动/常量） |
| `const_expr.rs` | 常量表达式与字面量（整数按类型 `of_int`、浮点按 IEEE754 拆分、bool、char、字符串） |
| `func_ref.rs` | 函数引用：普通函数、内建函数（`Box::new`、`ptr_from_parts_*`）、trait 方法（经 trait 实现表解析） |
| `rvalue.rs` | 右值：借用、二元/一元运算、聚合构造（结构体/枚举/元组），以及"收集右值引用的局部变量" |
| `spec.rs` | 规格闭包：把捕获变量用 `let` 绑定后翻译闭包体（前置/后置条件、契约断言、量词共用） |
| `stmt_expr.rs` | 语句 → 可执行代码（`Expr`）：赋值、调用、`if`/`match`、`loop` + 异常实现 `break`/`continue`/`return`、断言、契约断言 |
| `stmt_term.rs` | 语句 → 逻辑项（`Term`）：规格中引用函数体时使用，基本块被翻译成 lambda，控制流用 `let` + 嵌套 `if` 表示，量词在此翻译 |
| `decl.rs` | 函数（有函数体 → `let`，`extern`/`intrinsic`/`opaque` → `val`，递归组 → `let rec`）、全局常量、声明组分派，以及中间结构 `FuncData` |

## 4. 当前支持的翻译

### 4.1 语句（`ctx/stmt_expr.rs`、`ctx/stmt_term.rs`）

| Rust 构造（LLBC `StatementKind`） | 说明 |
| --- | --- |
| 赋值 `Assign`、`let` 绑定 | `x = rvalue`，顺序执行翻译成 `Sequence`，赋值翻译成 `Assign`/`let` |
| 调用 `Call` | 直接调用、trait 方法调用、函数指针调用；结果写入目标位置 |
| `return` | 可执行代码中 `raise Return`；规格中绑定返回值变量 |
| `if`/`else` | `Switch::If` |
| `match` | `Switch::Match`，翻译成对判别值的等值比较链；但判别式读取 `Rvalue::Discriminant` 尚未实现，所以枚举上的 `match`/`if let` 目前还用不了 |
| `loop`/`while` | `Loop` + `Break(level)`/`Continue(level)`，用 `while true` 与 `Break'`/`Continue'` 异常实现 |
| `assert!`/`debug_assert!` 与溢出检查断言 | `Assert`：`if cond then () else absurd`（`expected=false` 时取反） |
| `abort`/不可达 | `Abort` → `absurd`（逻辑项中为 `false`） |
| 契约断言 | `ContractAssert` → WhyML `assert`/`assume` + 规格闭包（由 `trust2-contract` 宏生成） |
| 规格量词 | `Quant`（`forall`/`exists`）→ WhyML 量词，仅在规格中出现 |
| 生命周期标记 | `StorageLive`/`StorageDead`/`Drop`/`PlaceMention` 被忽略（`Drop` 处标注为 Hack） |

### 4.2 表达式与右值（`ctx/rvalue.rs`、`ctx/place.rs`、`ctx/const_expr.rs`）

| 类别 | 支持情况 |
| --- | --- |
| 读变量、移动/拷贝 | ✅ `Operand::Copy`/`Move` + `Rvalue::Use` |
| 常量表达式 | ✅ 字面量、`const` 调用、函数引用 |
| 字面量 | ✅ 整数（按类型调用 `I32.of_int` 等）、bool、char、字符串；✅ 浮点（按 IEEE754 拆成尾数 + 二进制指数，非有限值未实现）；❌ 字节串 |
| 二元运算 | ✅ 按位 `^`、`&`、`\|`，比较 `== != < <= > >=`、`cmp`，算术 `+ - * / %`（Panic/UB → `*_strict`，Wrap → `*_wrapping`），`checked_add/sub/mul` → `*_overflowing`，移位 `<< >>`（严格版/无界版）；❌ 指针 `offset` |
| 一元运算 | ✅ `!` → `bit_not`、`-`（Wrap）→ `neg`；❌ `as` 强制转换 |
| 聚合构造 | ✅ 结构体字面量 → 记录、枚举构造子（元组式/结构体式）、元组；❌ 数组/重复/原始指针聚合 |
| 引用 | ✅ `&x`（共享、`UniqueImmutable`，引用被翻译为透明/解引用取值）；❌ `&mut`（`todo!()`） |
| 位置投影 | ✅ 结构体字段、枚举字段（走生成的访问函数）、元组字段（按需生成 `get_tupleN''_i`）、`*p` 解引用、全局量；❌ `PtrMetadata`、数组/切片索引与 subslice |
| 空元运算 | ❌ `size_of`/`align_of`/`offset_of`/`ub_checks`/`overflow_checks`/`contract_checks` 均未实现 |
| 其他 | ❌ `Rvalue::Discriminant`、`Rvalue::Len`、原始指针 `*const/*mut` |

### 4.3 类型（`ctx/ty.rs`、`ctx/type_decl.rs`）

| 类别 | 支持情况 |
| --- | --- |
| `struct` / `enum` | ✅ 记录类型 / 代数类型（仅无字段的 C 风格变体；带字段的变体会报 `unsupported enum variant with fields`，因为 why3-ptree 的打印机不输出无名构造子参数） |
| `Opaque` / `type` 别名 | ✅ 抽象类型 / 不生成声明 |
| 基本类型 | ✅ 整数（`i8..i128`、`u8..u128`）、`bool`、`char`、字符串、`()`、`!`（空类型） |
| 元组、`Box<T>`、引用、函数指针 | ✅（`&mut T` 表示为值对 `(新值, 原值)`） |
| 泛型 | ❌ 目前要求非泛型（`assert!(generics.types.is_empty())`） |
| 其他 | ❌ 浮点类型、数组、切片、`dyn Trait`、`impl Trait`、原始指针、联合体（报错） |

### 4.4 函数与全局量（`ctx/decl.rs`、`ctx/func_ref.rs`）

- ✅ 普通函数：有函数体 → WhyML `let`，局部变量用 `ref` 模拟可变性，函数体翻译成可执行代码
- ✅ 无可见函数体（`extern`/`intrinsic`/`Opaque`/`Missing`）→ 抽象声明 `val`
- ✅ 递归函数组 → `let rec`（**没有生成 `variant`**，见 §7）
- ✅ 全局常量 → `let`
- ✅ 内建函数：`Box::new` → `box_new`，`ptr_from_parts`（shared/mut） → `ptr_from_parts_shared/_mut`
- ⛔ trait 声明中的方法与 `unsafe` 函数被跳过（源码中标注为 Hack）；变参函数报错

## 5. 暂不支持 / 会失败的地方

遇到下列构造时，翻译器要么 `panic`（`todo!()`），要么返回 `Error`（打印 "unsupported ..."）：

- 语句：`CopyNonOverlapping`（函数体 `todo!()`）、内联汇编 `InlineAsm`；规格中禁止 `loop`/`break`/`continue`（`LoopInSpec`）、嵌套契约（`NestedSpec`）
- 右值：`&mut`/`TwoPhaseMut`/`Shallow` 借用、`as` 转换、`size_of` 系列、`Discriminant`、`Len`、数组/重复聚合、原始指针
- 字面量：字节串；浮点的非有限值
- 类型：泛型参数、浮点类型、数组/切片、`dyn Trait`、原始指针、联合体、带字段的枚举变体（`EnumVariantWithFields`）
- 其他：混合声明组（`MixedDeclGroup`）、原始字节常量（`RawBytesConst`）
- 终止性：`let rec` 与 `loop`/`while` 都不生成 `variant`，Why3 会插入必然失败的 `Termination` 目标（见 §7）

## 6. 已知缺陷（Bug 记录）

> 递归/循环的终止性属于功能缺口而非翻译 bug，单独见 §7。

### B1. 带字段的枚举变体无法翻译（`unsupported enum variant with fields`）

**现象**：`enum Shape { Empty, Line(i32) }` 报
`unsupported enum variant with fields: <crate>::Shape::Line`；如果绕过这层检查，生成的 WhyML 是
`type shape = | C_Empty | C_Line`（构造子参数被整个丢掉），why3 报
`Function C_Line expects no arguments`。

**根因**：Why3 语法树里构造子的字段只能表示成**无名参数**（`Param(loc, None, ghost, pty)`），
而 why3-ptree 0.3.0 的打印机（`~/.cargo/registry/src/*/why3-ptree-0.3.0/src/parser/mlw_printer.rs`
的 `pp_param`）只在参数满足 `ghost || opt_id.is_some() || ref` 时才输出内容，无名参数直接跳过。
另一条路（给参数起名字）也走不通：WhyML 把构造子的有名参数当作**类型的字段**，要求该类型的所有
构造子都声明同名字段，否则报 `Field x is missing in some constructors`。用本机 why3 1.8.2 实测：

| 写法 | 结果 |
| --- | --- |
| `type t = | A int | B bool` | ✅ |
| `type t = | A (x:int) | B (y:bool)` | ❌ `Field x is missing in some constructors` |
| `type t = | A | B (x:int)` | ❌ 同上 |
| `type t = | A (ghost : int)` | ❌ syntax error |

**当前处理**：`ctx/type_decl.rs` 在翻译阶段就报错（早失败，不再产出过不了类型检查的 `.mlw`）；
无字段变体（C 风格枚举）完全可用。

**修复路线**（都没做，等决定）：
1. 正确做法：给 why3-ptree 打补丁，让无名参数打印成 `| C int`（`pp_param` 里 `else` 分支打印
   `pp_pty(attr).marked`），需要在仓库里 vendor 这份依赖并在 Cargo.toml 加 `[patch.crates-io]`。
2. 权宜做法：把带字段的枚举编码成「抽象类型 + 未解释的构造子/访问函数」，不动依赖，但语义变弱
   （与载荷相关的性质都证不出来）。

### B2. 规格里的 `forall!` / `exists!` 捕获外层变量会生成未绑定变量

量词闭包若引用外层局部变量（例如后置条件里的返回值），生成的目标里会出现
`let _1'2 = _4'1 in`（`_4'1` 从未被绑定），why3 报 `unbound function or predicate symbol '_4'1'`。
涉及 `ctx/spec.rs::translate_spec_closure`（只绑定当前闭包的捕获）与
`ctx/stmt_term.rs::translate_quant`（只绑定量词自身）。用例：test10 早期版本的 `upper`/`double`。

### B3. 整数的 `overflowing_*` / `wrapping_*` 方法调用被翻译成无参 `val`

`a.overflowing_add(b)`、`i.wrapping_add(1)` 这类 core 方法会生成
`val overflowing_add''_core''_num''_'_u32'_ : (U32'.t, Bool'.t)`（**没有参数**），
而调用点照样传参，why3 报 `This expression has type (bv.BV32.t, bool), it cannot be applied`。
运算符形式（`+`/`-`/`*`）不受影响（会翻译成 `add_overflowing` 等库函数）。

### B4. 枚举上的 `match` / `if let` 不可用

判别式读取 `Rvalue::Discriminant` 是 `todo!()`（`ctx/rvalue.rs`），Charon 的 `Switch::Match`
在 `ctx/stmt_term.rs` 里按「判别式整数」比较翻译（实际类型是代数类型，会对不上），
整数 `match` 则落在 `Switch::SwitchInt => unreachable!()`。

### 已修

- **枚举构造子名非法**：拼接后的构造子名以类型名（小写）开头，WhyML 要求大写；
  现在 `name_map/build.rs::set_map` 拼接后再过一次 `ensure_u`（`Cshape''_Empty''_test10`）。
- **打印陈旧的证明记录**：why3find 若在证明前退出，`proof.json` 不会被更新，
  原来会直接把上一轮的结果打印出来；现在 `whyml_verify.rs` 会比较修改时间并打印 `Skipped`。

> 已知例子：`llbc_tests/test02` 的 LLBC 会在 `assert!(func_decl.generics.types.is_empty())`（`ctx/decl.rs`）处 panic，即该 crate 使用了当前不支持的泛型函数。

## 7. 递归与循环的终止性论证（功能缺口，不是翻译 bug）

一句话：**工具链没有把"终止性"所需的信息传给 Why3**。函数能通过类型检查、能生成 `.mlw`，
但验证时必然出现若干 `Stuck` 的 `Termination` 目标，且**换证明器、加超时都无济于事**（因为该目标的结论就是 `false`，见 7.3）。

### 7.1 现象

`llbc_tests/test10` 的当前结果：

```
Warning, file "test10.mlw", line 423, ...: termination of this expression cannot be proved,
                                             but there is no `diverges' clause in the outer specification
（共 4 条同样的 Warning）

File "test10.mlw", line 392, characters 9-28: proof failed
Goal Top.count_down''_test10: ✘ (5/6)
File "test10.mlw", line 432, characters 9-28: proof failed
Goal Top.even_count''_test10: ✘ (5/6)
File "test10.mlw", line 471, characters 6-24: proof failed
Goal Top.odd_count''_test10: ✘ (5/6)
File "test10.mlw", line 511, characters 5-23: proof failed
Goal Top.loop_scan''_test10: ✘ (2/3)
Theory test10.Top: ✘ (34/38)
```

| 目标 | `.mlw` 行 | Rust 侧来源 | 缺什么 |
| --- | --- | --- | --- |
| `Top.count_down''_test10` | 392 | `fn count_down`，落在 `let rec` 组里 | 递归函数的 `variant` |
| `Top.even_count''_test10` | 432 | `fn even_count`，与 `odd_count` **互递归**（`let rec … and …`） | 递归函数的 `variant` |
| `Top.odd_count''_test10` | 471 | `fn odd_count`（同一 `let rec` 组） | 递归函数的 `variant` |
| `Top.loop_scan''_test10` | 511 | `fn loop_scan`，`loop` + `break`/`continue` | 循环的 `loop variant` |

每个目标都只差最后 1 个子目标（如 `(5/6)`），差的正是 `Termination`；其余子目标（前置/后置条件）都已被 alt-ergo 证明。
why3 的 4 条 Warning 与这 4 个目标一一对应（Warning 指向源码表达式位置，goal 指向 VC 标签位置，所以行号不同）。

### 7.2 背景：WhyML 的 `variant` 与 effect

WhyML 中每个函数/表达式都带 **effect**，其中一个维度是"是否保证终止"：

- 保证终止 → effect 为 `Terminates`（`oneway` 的默认部分）
- 可能不终止（递归、循环、调用可能发散的函数）→ effect 含 **`diverges`**

"可能发散"本身不是错误，但 Why3 只在你能给出**良基测度**的情况下才接受"会发散"这件事：

```why3
let rec f (n : int) : int
  variant { n }            (* 测度表达式，不必是新变量，可以带 "with rel" 指定良基关系 *)
  = if n = 0 then 0 else f (n - 1)

while cond do
  invariant { ... }        (* 循环不变式 *)
  variant   { ... }        (* 循环测度 *)
  body
done
```

`variant { e }` 里 `e` 是测度；VC 生成器会对每个进入点生成一个 `Variant decrease` 义务
（"本轮的测度按良基关系严格小于上一轮"）。多个互递归函数需要**每个函数各写一个** `variant`（按字典序组合）。

### 7.3 根因：那 4 个目标在构造上就是 `false`

```ocaml
(* why3-1.8.2/src/mlw/vc.ml:609 *)
let check_divergence k =
  if diverges eff.eff_oneway && not env.divergent then begin
    Loc.warning warn_missing_diverges ?loc "termination of this expression \
      cannot be proved, but there is no `diverges' clause in the outer \
      specification";
    Kpar (Kstop (vc_expl loc attrs expl_divergent t_false), k)
  end else k in
```

- `expl_divergent` 就是 `Ident.create_attribute "expl:termination"`（`vc.ml:200`），所以这个子目标的名字就是 `Termination`。
- `t_false` 是字面量 `false`；`Kstop` 表示"只要证明这个公式"；`Kpar` 表示"与正常的 VC 并行"——于是它成为一个独立的、结论为 `false` 的义务。
- **为什么是 `false` 而不是真正的终止条件**：WhyML 的终止性不是 VC 里的某个公式，而是由 `variant` 机制在声明点单独生成 `Variant decrease` 义务。
  没有 `variant` 时，生成器手里**没有任何公式**可用，于是插入 `false` 当作"必须失败"的哨兵——而不是默许函数终止（那会不健全）。
- 所以这不是证明器能力/超时问题：证明 `false` 不可能，它**结构性不可证**。
- `not env.divergent` 是另一条出路：外层规格声明了 `diverges` 时（`vc.ml:1031` 通过属性 `vc:divergent` 把 `env.divergent` 置真），
  这个义务和 Warning 都不会出现——即把全正确性降级为部分正确性。

已用最小例子在 why3 1.8.2 上实测（`why3 prove -F whyml -a split_vc -a print_vc -P alt-ergo`）：

| 文件 | 内容 | 子目标 |
| --- | --- | --- |
| `/tmp/rec_novar.mlw` | `let rec f n = if n = 0 then 0 else f (n-1)` | Post ✔ / Pre ✔ / **Termination ✘** |
| `/tmp/rec_var.mlw` | 同上 + `variant { n }` | Post ✔ / Pre ✔ / **Variant decrease ✔**（`Termination` 消失） |
| `/tmp/rec_div.mlw` | 同上 + `diverges` | 只有 Pre/Post，**没有任何终止性义务** |
| `/tmp/while_novar.mlw`、`/tmp/while_var.mlw` | `while` 无/有 `loop variant` | 同上（有则变成 `Variant decrease`） |
| `/tmp/mutual_var.mlw` | `let rec … and …` 互递归 | 必须每个函数各给一个 `variant`，只给一个不够 |

`print_vc` 也能直接看到 VC 的形状：`o >= 0 /\ false /\ ...`。

### 7.4 影响面

- **递归**：`ctx/decl.rs` 的 `GDeclarationGroup::Rec` 分支生成 `Decl::Rec`，`Fundef` 的 `Spec.variant` 始终为空
  （`why3-ptree` 的 `Spec` 有 `variant : [(Term, Option<Qualid>)]` 字段，`ptree_helpers::empty_spec()` 默认空）。
- **循环**：`ctx/stmt_expr.rs` 的 `translate_loop` 生成 `ExprDesc::While(true, [], [], body)`——**invariant 与 variant 都是空数组**
  （`ExprDesc::While` 依次是 条件 / 不变式 / 测度 / 循环体）。
- **循环不变式也缺**（相关但独立的问题）：`while true` + `Break'` 异常退出的结构下，循环携带的性质无处书写。
  test10 里 `loop_scan` 的后置条件能证，只是因为它恰好写成 `result = n`（与循环体无关）。
- **`diverges` 会传染**：调用一个可能发散的函数，会让调用点的表达式 effect 也含 `diverges`；因此"只用 `diverges` 兜底"时需要沿调用图传播（见 7.5 路线 C）。
- 这与"翻译错误"不同：`.mlw` 本身类型正确，`cargo verify` 失败在证明阶段。

### 7.5 解决方向与需要做的事

按代价/收益排序。三条路线不互斥，C 可以今天先做，A 是正解，B 是长线。

#### 路线 C（止血，最小改动）：自动降级为部分正确性

让工具链对"需要终止性论证的函数"自动加 `diverges`，Why3 就不再生成 `Termination` 义务。

- 改动点：`ctx/decl.rs` 构造 `Spec` 时把 `diverge` 置 `true`（`ptree_helpers::empty_spec()` 里默认是 `false`）。
- 需要传播：`Spec.diverge = true` 只影响该函数自身的 `env.divergent`；调用它的函数其调用点 effect 仍含 `diverges`，
  所以要么对**所有**被验证函数统一置 `true`，要么在 `ctx/decl.rs` 里遍历函数体收集 `Call` 做一次调用图传播（传不彻底会冒出新的 `Termination` 目标，需实测确认）。
- 建议做成 CLI 开关（如 `--allow-divergence`），并在输出里明确提示"已按部分正确性验证"。
- 代价：后置条件从"一定返回且成立"弱化为"**若返回则**成立"。收益：test10 的 4 个 stuck 立即消失（与 `/tmp/rec_div.mlw` 实验一致）。

#### 路线 A（正解，注解驱动）：让用户写 `variant`

用户侧语法建议：

```rust
#[variant(n)]                     // 递归函数：测度表达式
fn count_down(n: u32) -> u32 { ... }

loop {
    loop_variant!(k - i);         // 循环测度
    loop_invariant!(i <= k);      // 循环不变式（配套缺口，见 7.4）
    ...
}
```

**A1. 先跑通（只动 `trust2-contract*` 与 `cargo-verify`，不改 Charon）**

1. `trust2-contract-macros/src/impl.rs` + `impl/parse.rs`：新增 `variant` 属性宏与 `loop_variant!`/`loop_invariant!` 函数式宏；
   属性宏把 `#[variant(e)]` 改写成函数体**首条语句** `trust2_contract::internal::variant(|| e);`（循环宏同理，注入循环体首句）。
2. `trust2-contract/src/internal.rs`：加占位函数 `pub fn variant<T: Fn() -> bool + Copy>(_: T) {}`（只为通过 rustc 类型检查）。
   这里**不需要改 Charon**：`recognize_spec_call`（`charon/charon/src/bin/charon-driver/translate/translate_functions.rs:61`）
   只认 `entry`/`precondition`/`postcondition`/`forall`/`exists`/`implies`/`old`/`contract_assert`/`contract_assume` 这几个名字，
   没列出的名字会被当成**普通 opaque 调用**留在 body 里，正好让 `cargo-verify` 自己去认。
3. `cargo-verify`：
   - `ctx/decl.rs`：`GDeclarationGroup::Rec` 分支里把该调用从函数体抽走（不生成语句），把闭包操作数翻成 `Term` 写进 `Fundef` 的 `Spec.variant`。
   - `ctx/stmt_expr.rs`：`translate_loop` 里把 `Box::new([])` 的两处换成从循环体抽出的 invariant/variant。
   - `ctx/mod.rs`：`Ctx` 增加"当前函数收集到的 variant""当前循环收集到的 invariant/variant"两个槽位；
     识别标记函数可参考 `ctx/func_ref.rs` 里按名识别内建函数的写法（配合 `prelude.rs::WILDCARD` 那套别名）。
4. 主要风险：body 里的是**运行时闭包**（Charon 会把它变成独立的 `FunDecl`），不是规格闭包，
   把它的 body 翻成 Term 需要处理"闭包捕获参数 → 当前作用域里的 `ref` 局部变量"的替换（`ctx/spec.rs::translate_spec_closure` 只服务规格闭包）。
   可行但要额外写一段映射逻辑；不想写就走 A2。

**A2. 干净做法（改 vendored Charon，约 4 处；`charon/` 就在本仓库内，是 path 依赖）**

1. `charon/charon/src/ast/expressions.rs:456`：`BuiltinFunId` 增加 `SpecVariant`。
2. `charon/charon/src/bin/charon-driver/translate/translate_functions.rs:61`：`recognize_spec_call` 增加 `"variant" => ...`。
3. `charon/charon/src/ast/gast.rs:244`：`FunSpecs` 增加 `variants: Vec<SpecClosure>`；
   `charon/charon/src/transform/collect_specs/unstructured_body.rs:~300` 收集该调用并从 body 中删除。
4. 循环：给 `ContractAssertKind` 增加 `LoopVariant`/`LoopInvariant`（或新增 terminator 种类），收集时挂 `spec_closure_id`；
   `cargo-verify` 侧按"最近的循环"归属——`Ctx` 现成的 `enter_loop`/`loop_depth` 机制可以直接复用。
5. 好处：完全复用现有规格闭包的捕获/命名管线（`ctx/spec.rs`、`name_map/locals.rs`），
   注解里的 Rust 变量由 Charon 解析成 `SpecClosure.captures`，不存在"字符串里的名字对不上"的问题，也不会有 A1 第 4 条的闭包映射难题。
6. 代价：动 `charon/`（编译时间变长），需要保证 LLBC 序列化兼容。

**最小验证步骤**：在动手改代码前，可以手工在生成的 `.mlw` 上验证机制——给 `let rec count_down` 加 `variant { U32'.to_int n_1'0 }`、
给 `loop_scan` 的 `while true` 加 `variant { ... }`，确认 `Termination` 变成 `Variant decrease`。这一步已在 7.3 的最小例子上验证过。

#### 路线 B（启发式合成，无需注解，长线）

- 结构递归：为（互）递归类型生成 `size_T : T -> int`（`ctx/type_decl.rs` 负责生成，互递归类型要一起生成），
  在 `GDeclarationGroup::Rec` 里挑一个"每次递归调用都严格变小"的参数，自动写 `variant { size_T arg }`。
- 计数循环：识别很窄的模式（如 `while i < n { i = i + 1 }`），从 LLBC 里 `BinaryOp(Add/Sub)` 的实参形状推出测度。
- 评价：鲁棒性差、要写一堆模式匹配；测度猜错只会**证明失败**（Why3 会报 variant 不成立），不会产生假证明，所以安全性可接受，
  但仍应排在 A 之后——因为 A 能把"推不出来的情况"交还给用户。

#### 其它相关待办

- **循环不变式通道**：即使不做 variant，要证明像样的循环后置条件也必须有 `invariant`（见 7.4）。它和 variant 共用同一条注解管线，建议一起设计。
- `why3-ptree` 的 `Spec.partial` 字段（抽象 `val` 的部分性）目前也未使用，和本节同属"库支持但生成器未用"的字段。

### 7.6 验收与回归

- `llbc_tests/test01` 的 `test01.mlw` 应与现有 `why3_out` **逐字节一致**（任何路线都不应改变无注解程序的输出；C 路线会改变，属于预期内的行为变更，需同步更新基线）。
- `llbc_tests/test10`：路线 C → 4 个 stuck 全部消失、整体 `✔`；路线 A/B → 4 个目标可证，或至少 `Termination` 变为 `Variant decrease` 并能被 alt-ergo 证掉。
- 新增用例建议覆盖：单递归（有/无注解）、互递归（每个函数各一个 `variant`）、`loop` 与 `while`（有/无 `loop variant`）、以及"测度写错"时应能失败（防止假证明）。

## 8. 如何扩展

- 新增**语句**支持：改 `ctx/stmt_expr.rs`（可执行代码）与 `ctx/stmt_term.rs`（规格中引用函数体），两处都要覆盖新的 `StatementKind`
- 新增**运算/右值**：改 `ctx/rvalue.rs`；新运算对应支撑库里的函数名（`trust2_contract/<type>.mlw`）
- 新增**类型**：`ctx/ty.rs`（类型语法）与 `ctx/type_decl.rs`（类型声明及其访问函数）
- 新增**规格注解通道**（如 `variant`、`loop invariant`）：宏在 `trust2-contract-macros/src/impl/`，Charon 侧在 `charon/charon/src/transform/collect_specs/unstructured_body.rs`，cargo-verify 侧在 `ctx/decl.rs`（函数）与 `ctx/stmt_expr.rs`（循环）；三条路线对比见 §7.5
- 新增**命名规则**：`name_map/` 下的 `names.rs`（名字字符串）、`keywords.rs`（大小写/关键字）
- 新增**支撑库函数**：`llbc_to_whyml/trust2_contract/*.mlw`；若需要新的库模块，在 `name_map/names.rs` 加别名并在 `prelude.rs` 的 `IMPORTS` 中登记
- 翻译过程中所有"上下文相关"的辅助（作用域栈、声明输出）放在 `ctx/mod.rs` 的 `Ctx` 上

## 9. 验证与回归

```sh
# 构建/安装
cargo install --path ./cargo-verify

# 端到端：在 llbc_tests/test01 下
cargo verify --charon-out-dir llbc_out --why3-out-dir why3_out
```

回归检查（无需 why3find）：用已有的 `llbc_tests/test01/llbc_out/*.llbc` 重新生成 WhyML，并与 `llbc_tests/test01/why3_out` 逐字节比对——拆分/重构后该输出应保持不变（`tuple.mlw` 的条目顺序因 `HashSet` 遍历顺序可能不同）。

```sh
export PATH=~/.rustup/toolchains/nightly-2026-06-01-x86_64-unknown-linux-gnu/bin:$PATH
cargo check -p cargo-verify --all-targets
cargo test  -p cargo-verify --bin cargo-verify   # 8 个 CLI 解析测试
cargo fmt   -p cargo-verify --check
```
