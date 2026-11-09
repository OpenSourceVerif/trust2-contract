**Prusti / Verus / Creusot：规约中的非 Copy 值**

研究于 2026-11-09 进行。本目录中的 Rust 源文件和工具日志是可复现的实验产物。未修改任何验证器的实现。

**结论**

当前的 Prusti 前端在针对非 Copy 值组合规约时存在真实限制。它无法透明地在捕获的程序值上、在解引用后的 `Ghost<T>` 上，或在非 Copy 类型的序列元素上调用已有的按值纯谓词。甚至一个独立谓词如果两次调用 `P(x)`，也无法通过 Rust 的移动检查。Verus 的 spec 代码和 Creusot 的 Pearlite 则有意允许这些逻辑使用，而不消耗可执行值。

这是一种架构设计选择，代价体现在源码 API 边界的表达力上。它不是逻辑值或 Viper 的必然限制。Prusti 已经具有不可变逻辑快照：它的 `Ghost<T>` 加上一个接受引用的谓词，成功表达了所测试的旧状态/当前状态性质。因此，该限制并不等同于无法表达历史值。

对于一个实现可用、且只观察普通不可变值的谓词，将其 API 改为接受 `&T` 可以保留其预期的逻辑含义。对于已有的不透明按值谓词 `P: T -> bool`，泛型适配器 `Q(x: &T) = P(*x)` 对非 Copy 的 `T` 会被拒绝。将整个库改为接受 `Ghost<T>` 或引用是一次重新设计，而不是透明地使用原 API。针对 `Seq<T>` 的历史谓词和泛型不变式谓词是这一区别的具体使用者。

内部可变性增加了单独的要求：可变内容必须通过变化的权限/状态或通过不变式来建模。复制一个 cell 的逻辑身份不得冻结其可变内容，也不得复制访问它们的权限。Verus 实验将不受限制的 spec 观察与线性 `PointsTo` 令牌分开。Creusot 实验使用 `PredCell` 的不变式 `Mapping<T, bool>`。

**版本**

| 组件 | 版本 |
|---|---|
| Prusti | 4536912ac5012094ecaed4e01fa8512f6bb47d63 |
| Prusti PCG 子模块 | e90a8144af97582508ff29e238dabfaa5f01800b |
| Prusti Rust | nightly-2026-08-15 |
| Viper | v-2026-09-01-0834 |
| Verus | release/0.2026.11.02.8bf68ee; 8bf68ee741df833ea5d0fe6a2255126fdea87e47 |
| Verus Rust | 1.99.1 |
| Creusot | 616cfccc7be6d17467e5ebee6d780353b9eab374 |
| Creusot Rust | nightly-2026-08-03 |
| Why3 | c369bc4cdc22d1e714255bb3675a2ce6b7242f19，由 Creusot 固定 |
| 用于 Creusot 的 Z3 | 4.16.0，来自 Verus 发行版 |

**观察结果**

这些实验中的所有 `Record` 类型都既没有 `Copy` 也没有 `Clone`。Prusti 和 Verus 使用整数负载；Creusot 使用布尔负载。这不改变 `Record` 的所有权性质。

| 实验 | 结果 | 日志 |
|---|---|---|
| Prusti 捕获的 `Record` 传给 `#[pure] by_value` | E0507 和 E0382 | prusti-direct.log |
| Prusti `by_value(*before)`，`before: Ghost<Record>` | E0507 | prusti-snapshot.log |
| Prusti `predicate! twice(x) = by_value(x) && by_value(x)` | E0382 | prusti-repeated.log |
| Prusti `by_value(*history[0])`，`history: Seq<Record>` | E0507 | prusti-collection.log |
| Prusti `Ghost<Record>` 旧状态 + `by_ref(&*before)`，随后修改 | 验证成功；退出码 0 | prusti-reference.log |
| Prusti 对被修改值的相反断言 | 在该断言处验证错误；退出码 1 | prusti-negative.log |
| Verus 重复按值谓词、旧快照、修改、之后的可执行移动 | 2 个已验证，0 个错误 | verus.log |
| Verus 对被修改值的相反断言 | 1 个已验证，1 个错误 | verus-negative.log |
| Verus `PCell<Record>`、重复 spec 使用 `PointsTo`、通过共享 cell 引用写入、`into_inner` | 2 个已验证，0 个错误 | verus-cell.log |
| Creusot 重复按值谓词、旧快照、修改、之后的可执行移动 | 生成 Coma；`vc_example` Valid | creusot.log, creusot-proof.log |
| Creusot 带按值不变式的 `PredCell<Record>`，通过共享引用替换，`into_inner` | `vc_interior_mutability` Valid；导入的契约 VC 也 Valid | creusot-proof.log |
| Creusot 对被修改值的相反断言 | `vc_example` Unknown (sat)，退出码 2 | creusot-negative-proof.log |

Creusot 的负例结果使用 Why3 的精确术语记录。正例客户端证明使用 `PredCell` 的标准可信规约，而不是重新验证其 unsafe 实现。Verus 的 cell 示例同样使用标准 `PCell` 接口。

**相关源码位置**

- Prusti 宏：`/home/xks/repos/prusti/prusti-contracts/prusti-specs/src/rewriter.rs`，`process_prusti_expression`；Fn 约束：`prusti-contracts/prusti-contracts/src/lib.rs`，`spec_block`。
- Prusti pure 注解：`prusti-contracts/prusti-specs/src/lib.rs`，`generate_for_pure`；它不改变 Rust 参数类型或所有权语义。
- Prusti `Ghost<T>` 的 `Deref` 返回 `&T`：`prusti-contracts/prusti-contracts/src/lib.rs:629`。逻辑实现为 `prusti-encoder/src/encoders/builtin/prusti.rs:569`，`encode_ghost`。
- Prusti 引用快照保留被引用者地址、元数据和值：`prusti-encoder/src/encoders/ty/pure.rs:159`，`TyPureImmRefData`。在该表示中，`T` 和 `&T` 并非普遍可互换。
- Creusot 显式地将非线性规约代码从借用检查器中隐藏：`/home/xks/repos/creusot/creusot/src/cleanup_spec_closures.rs:50`。`mir_built` 覆盖：`creusot/src/callbacks.rs:77`。THIR 提取：`creusot/src/translation/pearlite/from_thir.rs:56`。
- Creusot 将共享引用降为指向对象的逻辑类型：`creusot/src/backend/ty.rs:121`。可变引用使用单独的模型。这是一种语义抽象，并不是声称所有 Rust 引用观察都等于值观察。
- Creusot `PredCell<T>` 存储逻辑 `Mapping<T, bool>` 不变式：`creusot-std/src/cell/predcell.rs:14`；`new`、`replace` 和 `into_inner` 都依据该不变式给出规约。
- Verus 发行版源码：`tools/verus-source/source/rust_verify/src/erase.rs:117`，`mode_to_var_erase`。Spec 观察被擦除或由影子检查表示；exec 和 proof 使用被保留。
- Verus `PCell` 的 `PointsTo<T>::value` 返回 `&T`，实验显式解引用它，以获得非 Copy 的逻辑 `T`。源码：`tools/verus-source/source/vstd/cell/pcell.rs:115`。`write` 在第 218 行接受一个被追踪的可变权限引用。

**论文与官方文档**

- [The Prusti Project: Formal Verification for Rust, 2022](https://pm.inf.ethz.ch/publications/AstrauskasBilaFialaGrannanMathejaMuellerPoliSummers22.pdf)：第 4.1–4.2 节明确描述复用 Rust 的类型和借用检查器进行规约；第 3.3 和 4.4 节描述逻辑快照。这支持架构解释，而检出的代码决定当前行为。
- [Creusot: a Foundry for the Deductive Verification of Rust Programs, 2022](https://jhjourdan.mketjh.fr/pdf/denis2022creusot.pdf)：第 5 页明确区分 Pearlite 的类型检查与借用检查，并允许会违反 Rust 所有权的逻辑使用。下载文本：creusot-paper.txt。
- [Verus: Verifying Rust Programs using Linear Ghost Types, 2023](https://arxiv.org/html/2303.05491v1)：第 2.1 节将不受限制的规约与线性证明资源分开；第 5 节通过权限状态或不变式限制的读取来解释内部可变性。
- [Verus ghost code vs. exec code](https://verus-lang.github.io/verus/guide/ghost_vs_exec.html)：说明 ghost 代码中可以复制非 Copy 值，包括具有私有字段的抽象类型。
- [Creusot snapshots](https://guide.creusot.rs/snapshots.html) 和 [logic functions](https://guide.creusot.rs/logic_functions.html)。

**复现**

Prusti 在其仓库根目录下，初始化其固定的 PCG 子模块后，使用 `./x.py build` 构建。`VIPER_HOME` 指向下载的固定 Viper 后端。未使用 `cargo clean`。

从 `/home/xks/repos/prusti` 运行每个前端用例，使用以下命令，并将 `direct` 替换为 `snapshot_value`、`repeated` 或 `collection_value`：

```sh
VIPER_HOME=/tmp/spec-value-study/tools/viper/backends \
Z3_EXE=/tmp/spec-value-study/tools/viper/z3/bin/z3 \
PRUSTI_VERIFY_MODE=compile-only \
./x.py run --bin prusti-rustc -- --edition=2021 --cfg direct \
  /tmp/spec-value-study/prusti.rs -o /tmp/spec-value-study/prusti-direct
```

要进行完整验证，移除 `PRUSTI_VERIFY_MODE`，设置 `PRUSTI_CHECK_OVERFLOWS=false` 和 `PRUSTI_EXTRA_VERIFIER_ARGS=--timeout=90`，并使用 `--cfg reference`。对于相反断言，添加 `--cfg negative`。

Verus：

```sh
/tmp/spec-value-study/tools/verus-x86-linux/verus /tmp/spec-value-study/verus.rs
/tmp/spec-value-study/tools/verus-x86-linux/verus --cfg negative /tmp/spec-value-study/verus.rs
/tmp/spec-value-study/tools/verus-x86-linux/verus /tmp/spec-value-study/verus-cell.rs
```

Creusot，从 `/tmp/spec-value-study/creusot` 运行：

```sh
/home/xks/repos/creusot/target/debug/cargo-creusot creusot --only coma \
  --creusot-rustc /home/xks/repos/creusot/target/debug/creusot-rustc
```

负例变体使用 `--output-dir /tmp/spec-value-study/creusot-negative -- -F negative`。用以下命令证明相应生成文件：

```sh
/tmp/spec-value-study/tools/why3-install/bin/why3 prove \
  -C /tmp/spec-value-study/why3.conf \
  -L /home/xks/repos/creusot/target/creusot/packages/creusot \
  -P 'Z3,4.16.0,' -t 5 \
  /tmp/spec-value-study/creusot/verif/spec_value_study_rlib/example.coma \
  /tmp/spec-value-study/creusot/verif/spec_value_study_rlib/interior_mutability.coma
```

已有的 Why3 1.8.2 无法理解 Creusot 当前的 prelude（缺少 `min_sint`）。上述成功结果使用在 `tools/why3-install` 下单独构建的 Creusot 固定 Why3 提交。未替换系统 Why3 安装。