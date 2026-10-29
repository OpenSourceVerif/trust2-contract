# ULLBC-PCG：CFG 分析与 rustc 借用信息

独立读取 Charon `ullbc_ast::ExprBody` 及对应的 rustc NLL 借用信息并构建 PCG。当前算法按固定版本 MIR-PCG
移植标量/直接引用子集；运行时不链接、不执行 MIR-PCG，也不读取其分析结果。
具体源码对应、适配理由和未移植部分见 [ALGORITHM.md](ALGORITHM.md)。

```text
Rust → Charon --export-borrowck → ULLBC + 借用信息
  → 输入校验与分析上下文 → ULLBC CFG 活跃性 → 初始化 PCG
  → 基本块工作队列 → 每个操作四阶段转移 → 块入口/出口状态
  → analysis.json + 每个动作/阶段的 DOT → React 网页
```

## 运行

在工作区 `repo/` 中：

```sh
rustup toolchain install nightly-2026-06-01 --profile minimal --component rustc-dev
bash ullbc-pcg/scripts/run-case.sh          # 重新提取并分析
bash ullbc-pcg/scripts/run-case.sh --fixture # 已有 ULLBC 时跳过提取
cd ullbc-pcg/visualization
pnpm install --frozen-lockfile
pnpm run build
pnpm run serve
```

浏览 <http://127.0.0.1:5180>，需要 Node.js 和 pnpm。服务只绑定本机。
生成的 `data/`、`dist/`、`node_modules/` 不纳入版本控制。

也可直接分析已有文件：

```sh
cargo +nightly-2026-06-01 run --locked -p ullbc-pcg -- \
  ullbc-pcg/tests/fixtures/mutable_borrow.ullbc \
  --function mutable_borrow --output ullbc-pcg/visualization/data
```

函数名允许完整路径或唯一短名称。分析失败不导出成功结果。

## 案例与预期

```rust
pub fn mutable_borrow() -> i32 {
    let mut x = 0;
    let r = &mut x;
    *r = 1;
    x
}
```

| 位置与阶段 | 按原 PCG 规则产生的变化 |
| --- | --- |
| `r = &mut x` / PreOperands | 要求 `x: E` |
| 同上 / PostOperands | 撤去 `x` 的权限；此时尚无新借用边 |
| 同上 / PreMain | 要求并准备 `r: W` |
| 同上 / PostMain | `r: E`，加入 `x → r 的 lifetime projection`；不提前展开 `*r` |
| `*r = 1` / PreMain | 按需加入 `{r, r 的 lifetime projection} → *r`；`r: e`，`*r: E → W` |
| 同上 / PostMain | `*r: E` |
| `_0 = copy x` / PreOperands | 先收起解引用边，再回收已死引用的借用边，恢复 `x: E`，再要求读取权限 |

`E` 是独占权限，`R` 是读权限，`W` 是写权限（也用于覆盖前准备），`e` 是
ShallowExclusive，`∅` 表示无权限。初始化状态单独保存；不能单凭暂时的 `W` 判断
原值是否已被 move。权限偏序与原 PCG 相同，包括 `R` 和 `e` 不可比。

点击 ULLBC 语句选位置，右侧显示 Initial 和四个阶段。每阶段内每个实际动作均可点击，
显示的是该动作执行后的快照；无动作的阶段仍保留边界。`q`/`a`、Previous/Next
逐步遍历动作和阶段。点击源码可定位语句，重复点击在同位置的语句间切换。
JSON schema 为 3；包含真实 CFG 边、块入口/出口状态、逐点 NLL loan 集合和四阶段查询记录。网页中的图全部从分析结果导出，没有案例专用图。

PCG 绘图保留 ULLBC 的 `_0`、`(*r)`、区域编号和程序点，左侧语句由最终 ULLBC 原样格式化。
仅图的连线采用 MIR-PCG 约定：解引用超边画为两条绿色箭头；在直接借用的解引用节点存在时，
派生灰色别名虚线。JSON 中仍是一条 Deref 超边，不增加实际动作或 loan。
橙色边显示 rustc loan 编号、种类和区域；悬浮提示显示 ULLBC 创建位置。蓝色节点仍使用 ULLBC
区域编号，快照阶段（如 PreMain）放在悬浮提示中；右侧真实动作与四阶段导航保留。


## 代码入口

| 文件 | 职责 |
| --- | --- |
| `src/pcg/phase.rs`、`capability.rs`、`triple.rs` | 移植的阶段、权限偏序和操作前后条件 |
| `src/input.rs`、`liveness.rs` | Charon 输入验证、操作适配和引用活跃性 |
| `src/graph.rs` | Place、lifetime projection、Borrow/Deref/BorrowFlow 和独立 owned 状态 |
| `src/transfer.rs` | 按阶段执行 require/ensure、obtain、pack、引用流动及动作快照 |
| `src/analysis.rs`、`results.rs` | CFG 工作队列、入口/出口状态、四阶段调度和结果模型 |
| `src/borrowck.rs` | 借用元数据校验、loan 作用域和 outlives 查询 |
| `src/export.rs`、`main.rs` | 文件读取、Charon pretty-printer、JSON/DOT 导出 |
| `visualization/` | MIR-PCG 网页布局适配；见其 THIRD_PARTY.md |
| `tests/` | 真实 ULLBC、规则预期与非法输入回归测试 |

Charon 已扩展借用信息导出，AST 版本为 **0.1.228**。旧 `.ullbc` 需要重新提取：

```sh
bash ullbc-pcg/scripts/regenerate-fixtures.sh
```

`--ullbc --export-borrowck --mir promoted` 执行正常 Charon 转换，并将借用信息适配到最终 ULLBC；
元数据的位置、local 和 loan 创建位置均对应导出的 ULLBC。缺少借用信息、指纹不匹配、
位置缺失或不支持的适配会显式报错，不能退回空借用集合。
新的元数据契约是 `ullbc-final-borrowck-v2`；旧冻结版本必须重新提取。
未指定 `--ullbc` 时仍正常生成 LLBC，但 LLBC 借用信息尚未适配，会明确标记不可用。

## 范围与验证

框架按 CFG 保存并传播状态，活跃性使用反向工作队列直到不动点。
本轮 PCG 语义支持 goto 基本块链、标量/unit、非逃逸直接共享/可变引用、引用移动与共享复制。
提供 join 入口，但目前仅接受相同状态；不同状态的合流及循环抽象尚未实现，循环在分析前拒绝。
调用、分支/循环、引用参数/返回、reborrow、聚合投影和原始指针明确报 Unsupported。
通用 obtain/unblock 也尚未移植：例如同一语句内最后一次读取引用并写回其源变量，
可能被保守拒绝。这是实现缺口，不能归因于 ULLBC 天生不支持。
本版不证明与完整 MIR-PCG 等价，不计算具体返回值，不分析 LLBC。

```sh
cargo +nightly-2026-06-01 test --locked -p ullbc-pcg
cargo +nightly-2026-06-01 clippy --locked -p ullbc-pcg --all-targets -- -D warnings
cd ullbc-pcg/visualization
pnpm run typecheck
pnpm run build
```

测试检查借用边出现的阶段、按需展开/收起、移动引用的旧值投影和依赖保留、权限偏序、
动作快照连续性、构造 CFG 的跨块借用、重复/连续借用、unit 返回，
以及丢失位置、错配 loan、旧元数据、作用域/outlives 查询被篡改时的拒绝行为。
`cfg_borrows.rs` 为自建案例；没有改写上游测试。预期来自对应源码规则，
不是把本实现生成的快照作为正确性依据；测试不运行 MIR-PCG。

此前校验（2026-10-26，算法框架初版）：ULLBC-PCG 25 项测试通过，Charon 新增 2 项提取测试通过；
两侧 Clippy、前端类型检查/构建及原始案例浏览器验证通过。Charon Rust 全套测试中
`monomorphization/issue-708-from-str-mismatched-generics.rs` 在 `alloc::RawVecInner::finish_grow`
翻译处失败（该用例不启用借用导出模式），其余已运行测试通过；没有把全量检查记为通过。
OCaml 类型/运行测试因本机缺少 `unionFind`、`visitors.ppx`、`ppx_deriving.show` 等依赖未完成；
已运行 AST 生成，并对更新的 ML 源码做了解析检查。

最终 ULLBC 适配重做校验（2026-10-26）：25 项 PCG 测试通过；Charon 的 5 项借用导出测试通过，
覆盖正常/带借用信息输出的最终 body 一致性、存储插入、块合并、local 清理、未支持转换拒绝与正常 LLBC 生成。
两侧 Clippy、前端类型检查/构建及默认样例网页核对通过。完整 Rust 测试仍在上述单态化用例失败；
`make test` 因本机 PATH 中没有 `dune` 停在 OCaml 构建处，不能记为全量通过。
