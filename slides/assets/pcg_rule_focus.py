"""One teaching point per slide; complete definitions stay in term dialogs."""

# Each entry contains a heading, a resource rule, and its analysis provenance.
FOCUS = {
    'borrow-create': (
        'Borrow 边：区分引用槽与借用目标',
        'I(p,v) := cell 1 p i32_val v  // p 处整数值为 v\n'
        'G(p,v) := GenericOwn I32 p v  // 抽象目标资源\n'
        'Ref(s,p) := DirectMut I32 s (p,unit) // 引用槽\n'
        'DirectMut T s (p,m) :=\n'
        '  cell 1 s ref_addr p ** cell 1 s ref_meta m\n'
        '  ** !!(meta_type(m) = Metadata(T))',
        'Borrow 边记录借用依赖；Ref 只持有引用槽 s，G 持有目标 p。'),
    'deref': (
        'Deref Add：打开目标资源',
        '{{G(q,v)}} concrete q {{I(q,v)}}',
        'q=addr(x)。PCG 为访问 *x 添加 DerefEdge；Prusti 按 i32 类型选择 make_concrete，将抽象目标资源打开，保持值 v。'),
    'weaken': (
        'Weaken：交出旧值资源',
        '{{I(q,old_value)}} consume {{emp}}',
        'Weaken(..., W) 中 W 表示写能力；动作交出旧资源。随后 I(q,4) 中的 4 来自 MIR 写入，不是 PCG 图推导出的数值。'),
    'borrow-end': (
        'Borrow Remove：恢复借出目标',
        '{{G(q,v)}} concrete q {{I(q,v)}}',
        'PCG 回收叶边后，Prusti 恢复 assigned_ref 的目标。q=addr(x)=loc(a)，因此恢复的是 a；值来自当前状态。'),
    'field-open': (
        'Place expansion：暴露字段资源',
        'U(p,v) := cell 1 p u32_val v // u32 字段资源\n'
        'Own_T(q,s) := U(field(q,f),s.f)\n'
        'R_T(q) := EX s, Own_T(q,s) // 隐藏字段快照\n'
        '{{Own_T(q,s)}} unfold {{U(field(q,f),s.f)}}',
        'PCG 展开字段投影时触发 unfold。field(q,f) 是字段的逻辑地址；s.f 是快照 s 中的字段值。'),
    'field-close': (
        '投影收拢：重建结构体资源',
        '{{U(field(q,f),v)}} fold Own_T\n'
        '  {{Own_T(q,{f:=v})}}',
        'PCG 收拢字段投影时，Prusti 按结构体类型 fold。用当前字段值 v 重建快照 {f:=v}；本例写入后 v=4。'),
    'reborrow-chain': (
        '回收依赖：先还最内层借用',
        'ready(e) := 每个 blocker 均不被剩余边阻塞\n'
        'z → y → x → a',
        'PCG 按依赖选择可回收边；箭头表示归还顺序。close / return_to 分别指 generic / concrete，普通重借用还经过 pack/remove 更新。'),
    'basic-wand': (
        '跨函数依赖：生成归还 wand',
        'R(p) := EX v, GenericOwn I32 p v // 隐藏目标值\n'
        'W(q,p) := R(q) -* R(p)\n'
        '返回资源：R(q) ** W(q,p)',
        '输入、输出生命周期投影的依赖决定归还方向：交回结果 q 的资源，才能恢复输入 p。R 是对应目标的抽象资源。'),
    'call-edge': (
        'coupled Add：接收调用契约',
        'G_Point(p,s) := GenericOwn Point p s\n'
        'P(p) := EX s, G_Point(p,s)\n'
        'W(q,p,y0) := ALL z,\n'
        '  G(q,z) -* G_Point(p,(z,y0))',
        'G_Point 是 Point 的抽象资源，P 隐藏其快照；(z,y0) 是 x、y 的值。coupled Add 实例化调用依赖，返回资源与 W 来自方法后置条件。'),
    'package': (
        'UnblockGraph：给出 wand 的恢复计划',
        'W(q,p,y0) := ALL z,\n'
        '  G(q,z) -* G_Point(p,(z,y0))',
        'package 假设将来交回 G(q,z)，按解阻计划恢复 Point；未借出的 y 资源保留在包中。y0 与到期值条件来自方法规约。'),
    'apply': (
        'coupled Remove：使用归还凭证',
        'Point(p,(u,v)) := I(field(p,x),u) **\n'
        '                  I(field(p,y),v)\n'
        'G(q,72) ** W(q,p,y0)\n'
        '  |-- G_Point(p,(72,y0))',
        'Point 是展开后的两字段资源。PCG 移除 coupled edge 时，Prusti 经 call_wands 取出 wand 并 apply；72 来自 MIR。'),
    'history': (
        '历史节点：选择目标的旧快照',
        'addr_at(L,r) := (snapshot_at(L,r)).addr\n'
        'p := addr_at(_before_0_8, _3)',
        '历史基底指定 old 标签，旧引用快照定位目标。G_Point(p) / R_Point(p) 分别表示隐藏字段快照的抽象 / 展开资源；先按类型与 region 计算，再应用 old。'),
    'shared': (
        '共享借用：传播读资源',
        'Shared(T,slot,r) := RefShared(T,slot,r) **\n'
        '  GenericRead(T,r.addr,r.val)',
        '共享借用关系导向读资源视图，不产生独占目标资源。RefShared 持有引用槽；GenericRead 描述目标读权限与快照。值等式来自用户 ensures。'),
    'projection': (
        'LifetimeProjection：选择 region 的资源',
        'Indirect(cs,rho) := EX z,\n'
        '  GenericOwn I32 cs.x.addr z',
        'cs 是 c 的快照，rho 是 c.x 的生命周期。region 选择归属它的间接资源；本次 lifetime projection 展开分支不输出 fold/unfold。'),
    'flow': (
        'BorrowFlow：分析关系不必产生证明语句',
        '{{P}} skip {{P}}',
        '普通 BorrowFlow 与 FUTURE 记录借用依赖；此编码分支只输出注释，保持已有断言 P。Unsize 由专门分支处理。'),
    'unsize': (
        'Unsize：记录视图转换以供归还',
        'Seq(p,vs) := SEP i, I(elem(p,i),vs[i])\n'
        'Slice(p,n,vs) := !!(length vs = n) ** Seq(p,vs)',
        'elem(p,i) 是元素位置，SEP 遍历合法下标；Seq 持有这些元素，Slice 增加长度约束。MIR cast 转换视图，PCG 保存转换信息以供归还。'),
    'index': (
        '索引 expansion：分离目标元素与 frame',
        'Seq(p,vs) |-- I(elem(p,k),vs[k]) **\n'
        '  Frame(p,vs,k)\n'
        '适用条件：0 <= k < length vs',
        'PCG 的索引展开选择要访问的元素；其余元素保留为 Frame。k 与边界条件来自 MIR/类型，不能只靠图边证明。'),
    'unsize-undo': (
        'Unsize Remove：恢复数组视图',
        'Slice(p,n,vs) := !!(length vs = n) ** Seq(p,vs)\n'
        '{{Slice(p,n,vs)}} undo_unsize p n {{Seq(p,vs)}}',
        '移除 Unsize BorrowFlow 时触发 undo。PCG 保存的转换信息决定恢复方式；vs 使用当前内容，本例为 [13;3;72]。'),
    'branch': (
        '路径条件：守卫资源恢复',
        'guarded g P := if g then P else emp',
        'PCG 边携带的路径条件变成 guard_a / guard_b；只在实际走过的分支恢复对应目标。未借出的变量资源留在 frame。'),
    'loop-invariant': (
        '循环资源项：由分析选择需保留的位置',
        'I_resource := EX n, I(loc(i),n)',
        'used/live/initialized 与 PCG capability 共同决定资源项。n > 2 是用户 body_invariant 提供的值性质，不是 PCG 自动发现的。'),
    'loop-wand': (
        'LoopAbstraction：边界上的归还义务',
        'W(L) := G(addr_at(L,r)) -* R(loc(x))',
        '本页 G 隐藏抽象目标值，R 表示 x 的具体整数资源。successor actions 仅在 Add 且 edge_to_loop 时 package，Remove 时 apply；实跑 wand.not.found。'),
    'remote': (
        'Remote 基底：归还到函数入口资源',
        'ListRes(p) := EX v, GenericOwn List p v\n'
        'p_entry := addr_at(entry,self)\n'
        'W_remote := ListRes(q) -* ListRes(p_entry)',
        'ListRes 是链表类型 List 的抽象资源。Remote 基底按入口 old 解释；q 是当前链表位置的历史目标。本例权限不足，未证明归还义务。'),
    'inhale-path': (
        '不可达分支：先证明，再截断',
        '{{P}} check False {{P}}，须 P |-- !!False\n'
        '{{P}} assume False {{!!False}}',
        '这是 MIR panic 控制流的编码，不由 PCG 边生成资源。check 产生不可达证明义务；assume 截断后续路径，不能替代前一步证明。'),
}

# Line indices selecting the primary rule; the remaining definitions follow
# the explanatory paragraph. Cases without supporting definitions stay intact.
PRIMARY_LINES = {
    'borrow-create': [0, 1, 2],
    'field-open': [3],
    'basic-wand': [1, 2],
    'call-edge': [2, 3],
    'apply': [2, 3],
    'history': [1],
    'unsize': [1],
    'unsize-undo': [1],
    'remote': [2],
}


def presentation_parts(ident):
    lines = FOCUS[ident][1].splitlines()
    chosen = PRIMARY_LINES.get(ident, list(range(len(lines))))
    primary = '\n'.join(lines[i] for i in chosen)
    supporting = '\n'.join(line for i, line in enumerate(lines) if i not in chosen)
    return primary, supporting
