# 算法来源与移植边界

目标是把原 PCG 在当前案例所需的规则移到 ULLBC 上，而不是设计另一套“类似 PCG”的借用计数算法。
基准是工作区 `pcg/` 的提交 `6fdb05e6377da5316e5388320d2ddf3d13082f6e`。
以下上游路径均相对该 checkout；本地路径相对本 crate。

## 1. 直接保留的规则

| 上游源码 / 入口 | 本地实现 | 保留的内容 |
| --- | --- | --- |
| `src/pcg/domain/eval_stmt_phase.rs` | `src/pcg/phase.rs` | 四阶段枚举和顺序；去掉 rustc 展示/type-export 包装 |
| `src/pcg/capabilities.rs` | `src/pcg/capability.rs` | E/e/R/W、Debug 和完整偏序；仅改类型名与 serde 导入 |
| `src/pcg/triple.rs` / `TripleWalker` | `src/pcg/triple.rs` | 当前操作的前后条件；把 MIR visitor 改为匹配已验证 ULLBC Operation |
| `src/pcg/engine.rs` / `visit_all_phases`、`src/pcg/visitor/mod.rs` | `src/analysis.rs`、`State::phase` | 同一状态连续执行四阶段，operand/main 分开 require/ensure |
| `src/pcg/visitor/stmt.rs` / `stmt_pre_main` | `State::phase(PreMain)` | 赋值目标准备为 W |
| `src/pcg/visitor/assign.rs`、`src/borrow_pcg/state/mod.rs` / `add_borrow` | `State::phase(PostMain)` | 借用指向目标引用的 lifetime projection；引用赋值建立 BorrowFlow |
| `src/pcg/visitor/mod.rs` / operand visitor | `State::label_reference` | Move 后用 Before(PostOperands) 标记旧引用值 |
| `src/pcg/obtain/expand.rs` / `expand_place_one_level` | `State::obtain` | 按需展开解引用，先加边、更新权限，再标记被阻塞投影 |
| `src/pcg/place_capabilities.rs` / `update_for_deref` | `State::obtain` | 读展开为 R/R；可变写展开为 e/E |
| `src/pcg/visitor/pack.rs` / `pack_old_and_dead_borrow_leaves` | `State::pack` | 反复移除可回收叶边；存在下游依赖时不能因局部变量已死而删边 |
| `src/pcg/visitor/obtain.rs` / `remove_deref_edges_to`、`restore_place` | `State::pack`、`restore_owned` | 收起解引用、移除/取消标签、按 owned 初始化恢复权限；已有 R 不在 restore 中直接升级 |
| `src/owned_pcg/impl/update.rs` / `ensures` | `State::emit` | owned 初始化独立于借用权限；借用撤权限不等于移走值 |
| `src/pcg/visitor/obtain.rs` / `record_and_apply_action` | `State::emit`、`src/export.rs` | 执行动作后立即保存图；前端不推测中间状态 |

枚举和偏序是源码适配复用。Triple 是支持子集的源码移植。其余表项是按上游对应函数
移植的受限实现，**并非原函数整体搬运，也没有直接依赖上游 PCG crate**。
未涉及当前片段的分支不能据此视为已实现。上游一次批处理的动作在这里可能拆为多个
SetCapability/LabelProjection，快照粒度不承诺与上游网页逐帧完全相同。

操作规则如下，空后置条件表示不改变该权限：

| 操作 | operand 前置 → 后置 | main 前置 → 后置 |
| --- | --- | --- |
| `copy p` | R → 无变化 | 由外层赋值决定 |
| `move p` | E → W | 由外层赋值决定 |
| `&p` | R → R | 由外层赋值决定 |
| `&mut p` | E → 移除权限 | 由外层赋值决定 |
| `target = rhs` | 由 rhs 决定 | W → E |
| `StorageLive` | 无 | Unalloc → AllocateOrDeallocate |
| `StorageDead` | 无 | AllocateOrDeallocate → Unalloc |
| `Return` | 无 | Return 条件：当前片段返回位置须 E |

阶段顺序是：PreOperands 回收叶边并满足 operand 前置条件；PostOperands 执行 operand
后置条件和 move 旧值标记；PreMain 满足 main 前置条件并准备赋值目标；PostMain
执行 main 后置条件，再构建借用/引用流动边。不能把原先一个原子 transfer 的前后快照
简单重命名为四阶段。

## 2. 图结构为何改变

旧实现把 `r = &mut x` 直接画成 `x → *r`，并维护“持有者列表”。这绕过了原 PCG 的
lifetime projection、按需展开和旧值依赖，已删除这一路径。

新图在借用后只有 `Borrow(x, LP(r))`。使用 `*r` 才加入 `Deref({r, LP(r)}, *r)`。
`LP` 由引用 place、其类型中的 region、旧值标签及展开标签构成，不等于单独 region ID。
引用赋值加入 `BorrowFlow(LP(source), LP(target))`。移动后旧的 source 投影仍可保留，
直到没有活跃下游依赖；不把原借用边重定向到新的“持有者”。

回收从叶子开始，因此当前案例按 `Deref → Borrow → 恢复 x` 的顺序进行；有 BorrowFlow
时必须先处理下游流动边。可收起的解引用投影即使对应引用仍活跃也可收起，之后再次
使用时重新展开。并非每条删边动作都代表借用生命周期结束。

## 3. 确实由 ULLBC 输入造成的适配

- **AST、位置和类型**：使用 Charon LocalId/RegionId、Statement/Terminator 和 source span。
  `input.rs` 先验证类型与操作；本版只接受 Body region 的直接 `&scalar` / `&mut scalar`。
- **StorageLive**：Charon `ast/ullbc_ast.rs` 明确定义已分配时释放并重新分配，且插入 return local
  的 StorageLive。初始分配状态与 PreMain 的存储适配遵循此定义；重新分配前仍须取得权限。
  已释放位置的 StorageDead 按 Charon 定义无效果。
- **活跃性**：`liveness.rs` 从 ULLBC use/def 计算当前封闭直线路径上的引用活跃性，提供原
  pack 规则需要的 dead 查询。原 PCG 也有基于 MIR 的 liveness（`src/utils/liveness.rs`）；
  此处不伪造 rustc NLL facts。StorageLive 在 ULLBC 中重置旧存储，因此杀死旧局部值的活跃性。
- **区域关系**：普通 `.ullbc` 不提供原 rustc borrow-checker context。当前仅对已验证的
  `&scalar → &scalar` 赋值连接唯一 lifetime occurrence；不根据 RegionId 大小/相等猜一般
  outlives 关系。多个 lifetime occurrence、引用嵌套、跨过程关系不在此适配能力内。
- **Retag**：Charon 普通赋值附带 WithRetag；当前跟踪权限图，不解释指针 provenance tag。

以上输入适配不改变四阶段、权限偏序、当前操作 Triple 或 Borrow/Deref/Flow 的职责。

## 4. 尚未移植，不是 ULLBC 差异

通用 obtain/unblock（含同一语句内借用最后使用后写回 owner）、reborrow、嵌套投影、
函数摘要、region inference、CFG join/fixpoint、循环和路径条件均未移植。
不支持的 AST 会报 Unsupported；受限 obtain 无法满足的条件报 CapabilityUnavailable 并带阶段。
因此错误不等同于证明 Rust 程序非法；合法但超出 obtain 能力的输入仍可能被拒绝。

当前没有形式化等价证明，也没有用 MIR-PCG 运行结果做差分验证。测试根据上述源码规则
直接写出阶段/边/权限预期，并用非法 ULLBC 检查拒绝行为。这能防止已覆盖规则退化，
不能代替完整算法正确性证明。下一次扩展应继续逐条移植上游所需路径并补对应测试，
而不是增加与上游不一致的捷径。

## 来源说明

保留各移植文件的来源注释。`src/pcg/triple.rs` 保留上游 © 2023 ETH Zurich 和 MPL-2.0
声明，适用文件级条款见 <https://mozilla.org/MPL/2.0/>。其余来源与修改范围在上表列明。
网页来源及许可单独记录在 `visualization/THIRD_PARTY.md`。
