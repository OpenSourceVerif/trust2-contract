# ULLBC-PCG：第一个案例

独立读取 Charon `ullbc_ast::ExprBody` 并构建 PCG。当前算法按固定版本 MIR-PCG
移植标量/直接引用子集；运行时不链接、不执行 MIR-PCG，也不读取其分析结果。
具体源码对应、适配理由和未移植部分见 [ALGORITHM.md](ALGORITHM.md)。

```text
Rust → Charon → .ullbc → ULLBC-PCG 四阶段分析
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
JSON schema 为 2；网页中的图全部从分析结果导出，没有案例专用图。

## 代码入口

| 文件 | 职责 |
| --- | --- |
| `src/pcg/phase.rs`、`capability.rs`、`triple.rs` | 移植的阶段、权限偏序和操作前后条件 |
| `src/input.rs`、`liveness.rs` | Charon 输入验证、操作适配和引用活跃性 |
| `src/graph.rs` | Place、lifetime projection、Borrow/Deref/BorrowFlow 和独立 owned 状态 |
| `src/transfer.rs` | 按阶段执行 require/ensure、obtain、pack、引用流动及动作快照 |
| `src/analysis.rs`、`results.rs` | 四阶段调度和结果模型 |
| `src/export.rs`、`main.rs` | 文件读取、Charon pretty-printer、JSON/DOT 导出 |
| `visualization/` | MIR-PCG 网页布局适配；见其 THIRD_PARTY.md |
| `tests/` | 真实 ULLBC、规则预期与非法输入回归测试 |

工作区只增加独立 crate 及其依赖，Charon 内核未修改。绑定 Charon 0.1.227，参考提交
`08cf4caa4324857ec5e562366a00cc3b3630407c`。

## 范围与验证

支持单条执行路径（含 goto 链）、标量/unit、非逃逸直接共享/可变引用、引用移动与共享复制。
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
动作快照连续性，以及提前读写被借用源变量等反例。预期来自对应源码规则，
不是把本实现生成的快照作为正确性依据；测试不运行 MIR-PCG。
