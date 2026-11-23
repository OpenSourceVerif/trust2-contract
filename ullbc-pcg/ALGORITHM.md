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
  的 StorageLive。开启借用信息导出也执行相同的正常转换流程；元数据随插入的位置更新。初始分配状态与 PreMain 的存储适配遵循此定义；重新分配前仍须取得权限。
  已释放位置的 StorageDead 按 Charon 定义无效果。
- **活跃性**：`liveness.rs` 从 ULLBC use/def 在基本块 CFG 上用反向工作队列计算引用活跃性，提供原
  pack 规则需要的 dead 查询。原 PCG 也有基于 MIR 的 liveness（`src/utils/liveness.rs`）；
  此处不伪造 rustc NLL facts。StorageLive 在 ULLBC 中重置旧存储，因此杀死旧局部值的活跃性。
- **借用查询**：`Charon --export-borrowck` 调用 rustc consumer API，取得同一份 MIR 的
  `BorrowSet` / `RegionInferenceContext`，运行 rustc `Borrows` 数据流得到 primary effect 前后的作用域。
  记录转换时的 ULLBC 位置，再导出；分析器本身不链接 rustc。
  引用赋值只有在 rustc `eval_outlives(source, target)` 成立时才建立 BorrowFlow。
  ULLBC 的 `Region::Body` 和编译器 region 编号使用不同命名空间，以 local 的绑定表连接。
- **回收范围**：保留 ULLBC liveness 和依赖叶条件，并额外要求对应 loan 不在编译器作用域内才删除 Borrow 边。
  这是本轮保守约束，**不是声称原 MIR-PCG 的 dead 查询等同于 loan 不在作用域内**。
  上游 NLL 路径仍使用自身 place liveness；这里可能比完整上游更保守。
  rustc 的 before/after primary effect 不等于四阶段边界。目前 transfer 只在 PreOperands 使用 before；
  after 用于结果检查/展示，不能拿它冒充 PreMain 的编译器状态。
- **Retag**：Charon 普通赋值附带 WithRetag；当前跟踪权限图，不解释指针 provenance tag。

以上输入适配不改变四阶段、权限偏序、当前操作 Triple 或 Borrow/Deref/Flow 的职责。

## 4. 尚未移植，不是 ULLBC 差异

通用 obtain/unblock（含同一语句内借用最后使用后写回 owner）、reborrow、嵌套投影、
函数摘要、一般 PCG join、循环图抽象和路径条件均未移植。region inference 由 rustc 提供，
不在 ULLBC-PCG 中重新求解。CFG worklist、块入口/出口、变化检测、join 调用点以及活跃性不动点
已实现；PCG 的 join 只接受相等状态，输入验证明确排除分支和循环，不能据此宣称支持循环 PCG。
不支持的 AST 会报 Unsupported；受限 obtain 无法满足的条件报 CapabilityUnavailable 并带阶段。
因此错误不等同于证明 Rust 程序非法；合法但超出 obtain 能力的输入仍可能被拒绝。

当前没有形式化等价证明，也没有用 MIR-PCG 运行结果做差分验证。测试根据上述源码规则
直接写出阶段/边/权限预期，并检查修改 ULLBC 后拒绝旧元数据、修改 loan 作用域和 outlives 后决策随之改变。这能防止已覆盖规则退化，
不能代替完整算法正确性证明。下一次扩展应继续逐条移植上游所需路径并补对应测试，
而不是增加与上游不一致的捷径。

## 5. Charon 导出契约

`TranslatedCrate.borrowck: Option<BorrowckData>`，其中 `bodies` 的每个元素由 `function_id`
关联一个函数。函数级容器不会丢失语句信息：`points` 覆盖每条 ULLBC statement 和 terminator，
位置为 `(ULLBC BlockId, statement_index)`，terminator 的 index 等于该块语句数。

- `loans`：loan ID、ULLBC 创建位置、borrowed/assigned local、借用种类、编译器 region。
- `points`：before/after loan 集合；这些是 rustc 的作用域状态，不是全部 Polonius 关系。
- `reference_regions` / `outlives`：局部引用对应的编译器 region，以及这些 region 与 loan region 间已求值的 outlives 关系。
- `body_fingerprint`：导出 body 的确定性校验值，防止清理/编辑后的 body 与旧事实混用；不是防篡改签名。
- `unsupported`：适配器无法忠实表达时的显式原因。不会用空 facts 假装支持。

源码入口在 `ullbc_to_llbc` 之前运行，阶段标记为 `ullbc-pre-structure-borrowck-v2`。最终 ULLBC 文件的导出契约为 `ullbc-final-borrowck-v2`。从 borrowck body 的副本翻译，只擦除副本上的 region 名称，原 NLL body/context
保留用于查询。禁止 optimized MIR fallback；mono、precise-drops、multi-target 和 const 函数尚不支持。

正常 Charon 转换全部执行，不因借用信息改变 AST 或跳过清理。初始翻译记录 MIR 操作的 ULLBC 锚点；
转换中的位置更新由具体 pass 使用它实际执行的映射维护：存储标记与 unit 初始化插入、goto 合并、
返回块复制、不可达块清理、块重编号、local 重编号、Nop 删除。标量指针 metadata 和 literal/unit
常量规范化保持程序点；可能增加操作的其他常量降低明确 Unsupported。新插入的无 loan 效果操作
使用插入边界处下一条原操作的 before 集合，并标记 synthetic。不能用上一操作的 after 集合
替代这个集合，因为 NLL 可能在两者之间结束 loan。

每个 pass 后核查 body 指纹；没有适配器的变换如果修改了 body，借用信息会标记 Unsupported，
仍输出正常 Charon 结果。只有类型区域统一和注释恢复这两类已审核的纯位置保持 pass 直接刷新指纹。
一对多 MIR 翻译、被删除的 loan 创建以及其他未支持变换同样拒绝借用分析；不猜测对应关系。
PCG 导出拒绝旧的冻结契约。直接分析接口仍验证最终 body 指纹和每个 loan/point/local 的对应关系。

不要求 `--ullbc` 才能使用 `--export-borrowck`：未指定时，正常 LLBC 结构化仍执行。
当前 LLBC 的借用信息明确标记不可用，并清除 ULLBC 程序点，不能拿 ULLBC 位置冒充 LLBC 位置。

## 6. 本轮验收用例

原始 `mutable_borrow` 保持默认展示；另外自建 `repeated_write`、`sequential_borrows`、
`goto_borrow`（正常清理后合并为一个块）、`two_shared`、`unit_result`、`removed_unit_before_borrow`。已有的 shared/move/copy/scalar 参数
用例重新提取为带借用信息的输入。branch/cycle/reborrow 用例必须拒绝。测试不证明数值计算结果，
LLBC 只验证正常生成，未集成 LLBC-PCG、Why3 或函数契约证明。
另有求解器级测试把最终 ULLBC 在借用之后拆成两个块，并同步搬移借用信息，验证跨 goto 的状态传播；
该测试明确使用构造 CFG，不冒充 Charon 对该源程序的默认导出。

## 来源说明

保留各移植文件的来源注释。`src/pcg/triple.rs` 保留上游 © 2023 ETH Zurich 和 MPL-2.0
声明，适用文件级条款见 <https://mozilla.org/MPL/2.0/>。其余来源与修改范围在上表列明。
网页来源及许可单独记录在 `visualization/THIRD_PARTY.md`。
