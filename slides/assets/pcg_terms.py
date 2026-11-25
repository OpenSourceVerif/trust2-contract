"""Portable, explicit semantics for vocabulary used by the teaching notation."""
import html,json,re
E='evidence/pcg-encoding-20261125/'
M=E+'mutable/log/viper_program/program-check.vpr.html'
S=E+'shared/log/viper_program/program-check.vpr.html'

def term(title,definition,meaning,refs=(),dependencies=()):
    return dict(title=title,definition=definition,meaning=meaning,refs=list(refs),dependencies=list(dependencies))

TERMS={
'cast_array_to_slice':term('cast_array_to_slice：数组到切片的编码契约',
 '{{Seq(base,vs)}} cast_array_to_slice base (length vs)\n{{Slice(base,length vs,vs)}}.',
 '该名字概括 MIR Unsize 的资源视图转换，不是在 Rust cast 之后再执行一次转换。地址和当前序列保持，长度成为切片 metadata；序列视图与后端具体谓词的表示关系是单独接口。', [E+'arrays/log/viper_program/program-check.vpr.html#L2128'],[]),
'undo_unsize':term('undo_unsize：归还数组资源视图',
 '{{Slice(base,n,vs)}} undo_unsize base n {{Seq(base,vs)}}.',
 'PCG Unsize BorrowFlow 提供原来源和转换信息。该操作恢复数组视图，不恢复元素旧值；n 与 vs 的长度关系由 Slice 保持。', [],[]),
'open_index':term('open_index / close_index：元素与剩余资源',
 '{{Seq(base,vs) ** !!(0 <= k < length vs)}}\nopen_index base k\n{{I(elem(base,k),vs[k]) ** Frame(base,vs,k)}}.\n{{I(elem(base,k),v) ** Frame(base,vs,k)}}\nclose_index base k {{Seq(base,update(vs,k,v))}}.',
 'Frame 只含合法下标中除 k 外的元素。写入前交出目标元素旧资源，MIR 赋值建立新值，随后才能按更新后的序列重组。此规则是可移植序列视图，不是说 PCG 自动证明了所有序列性质。', [],['elem','SEP']),
'recover':term('recover：对应案例的有序归还脚本',
 'recover target := execute selected PCG removal actions\n  with their type-specific resource conversions.',
 '这是具体案例操作序列的简称，不是无前提的通用恢复公理。Borrow、Deref、coupled edge、LoopAbstraction 分别选择不同转换或 wand 操作，路径条件和已有资源必须满足。', [],['UnblockGraph','concrete','apply_wand']),
'Own':term('Own T：由类型选择的具体资源',
 'Own I32 p n := cell 1 p i32_val n.\nOwn T_struct p {f:=n} := cell 1 (field(p,f)) u32_val n.\nOwn Point p {x:=u;y:=v} :=\n  Own I32 (field(p,x)) u ** Own I32 (field(p,y)) v.',
 'Own 不是未定义的通用所有权：它按类型展开为该类型的字段权限和快照约束。这里只列已用到的整数与结构体；引用槽由 DirectMut/RefShared 表示。i64 需要 I64 对应的值字段、快照和转换方法，不复用 I32 的字段。', [M+'#L759',E+'reborrow/log/viper_program/program-check.vpr.html#L734'],['cell','field','Snap']),
'AnalysisLocation':term('MIR 位置 × PCG 分析阶段',
 'MIR Location = (basic_block, statement_index).\nPCG AnalysisLocation = (MIR Location, phase).\nPreOperands → capture operands → PostOperands\n→ PreMain → MIR effect → PostMain.',
 'bb0[6] 表示 MIR 基本块 bb0 的第 6 号语句（从 0 编号）；PreMain 是围绕该语句主效果的 PCG 阶段。两者定位同一个编码时刻，不能把 PreMain 当成 MIR 指令或 Rust 规约。Initial 是保存的初始图快照标签；terminator 与 successor actions 的调度另按控制流处理。第二页在 *x=4 的主效果前先展开目标，随后才消费旧值并写入 4。',
 [E+'source/prusti-encoder/src/encoders/mir_impure.rs.html#L2117',E+'points/mutable-test1-bb0-6-pre_main.json'],[]),
'cell':term('cell：一个逻辑字段的权限和值',
 'cell sh p f v h := p <> null /\\ 0 < sh <= 1 /\\\n  h = singleton (p,f) (sh,v).',
 'sh 是权限份额，1 表示完整写权限；p 是逻辑对象地址，f 是字段标识，v 是字段的当前值。i32_val/u32_val 简写输出的 p_Int_i32_val/p_UInt_u32_val，整数载荷按 Snap 的约定表示。它描述单个逻辑字段，不直接等于 C 内存块，也不直接等于 VST data_at。映射到 data_at 还需要类型布局、地址、份额与值的对应证明。', [M+'#L759'],['Assertion','Snap']),
'DirectMut':term('DirectMut：引用槽自身的资源',
 'DirectMut T s (p,m) :=\n  cell 1 s ref_addr p ** cell 1 s ref_meta m\n  ** !!(meta_type(m) = Metadata(T)).',
 'T 是目标类型；s 是存储引用的槽地址；p 是引用指向的目标地址；m 是指针 metadata。ref_addr/ref_meta 分别简写 p_Ref_mutable_addr/p_Ref_mutable_metadata。它只拥有槽 s 的这两个字段，并检查 metadata 类型；不拥有 p 处的目标内容，也不单独保证完整 Rust 引用有效性。第一个案例中 s=loc(x)、p=loc(a)、T=I32、m=unit；目标值 6 由 G(loc(a),6) 单独持有。', [M+'#L763'],['cell','Metadata','unit','loc','GenericOwn']),
'GenericOwn':term('GenericOwn：抽象目标资源及类型化快照',
 'GenericOwn T p v :=\n  ParamOwn(T,p) ** !!(param_snapshot(T,p) = box_T(v)).\n(* v : Snap T; ParamOwn is opaque. *)',
 'T 是目标类型、p 是目标地址、v 是当前内容的类型化快照。ParamOwn 对应实际 p_Param 的完整权限，该谓词没有可展开的主体；box_T 对应生成的 make_generic_s_T。make_generic/make_concrete 的方法契约在具体资源与抽象资源间转换并保持快照，不能把这两个谓词直接定义为逻辑等价。Point 的快照是字段组合，不能用单个整数代替。', [M+'#L770',M+'#L2924'],['Snap','generic','concrete']),
'GenericRead':term('GenericRead：抽象目标的读权限与快照',
 'GenericRead(T,p,v) :=\n  ParamRead(T,p,rho_read) **\n  !!(param_snapshot(T,p) = box_T(v)).',
 'ParamRead 是 p_Param 的后端读份额资源；rho_read 表示后端 read()，不是 Rust 的生命周期 region。该抽象资源不能未经读份额转换规则就替换为所有具体字段 cell 的组合。共享引用的值快照来自这个目标资源。', [S+'#L592',S+'#L510'],['GenericOwn','Snap']),
'RefShared':term('RefShared：共享引用槽的直接资源',
 'RefShared(T,s,r) :=\n  cell 1 s shared_addr r.addr **\n  cell 1 s shared_meta r.meta **\n  !!(meta_type(r.meta) = Metadata(T)).\nShared(T,s,r) := RefShared(T,s,r) **\n  GenericRead(T,r.addr,r.val).',
 's 是引用槽，r 是包含 addr、meta、val 的共享引用快照。RefShared 仅定义槽的字段；Shared 才把它和目标读资源组合起来。完整槽权限不意味着拥有目标的完整写权限。', [S+'#L592',S+'#L510'],['cell','GenericRead','Metadata','snapshot']),
'Shared':term('Shared：共享引用的完整资源视图',
 'Shared(T,s,r) := RefShared(T,s,r) **\n                 GenericRead(T,r.addr,r.val).',
 's 为引用槽地址，r.addr/r.meta/r.val 分别是目标地址、metadata 和目标值快照。get_x 的输入 T=Point；返回 T=I32。s.val.x 中的 s 在该案例是输入快照的变量名，不是这里形式参数 s 的槽地址，均由调用位置区分。', [S+'#L592',S+'#L640'],['RefShared','GenericRead']),
'Metadata':term('Metadata(T)：目标类型对应的指针元数据类型',
 'Metadata(T) := <T as Pointee>::Metadata.\nMetadata(I32) = Unit; Metadata([I32]) = Usize.',
 'Sized 目标（如 i32、Point）使用 unit 元数据；切片的 metadata 是长度。meta_type(m) 读取泛型快照上的类型标签，对应输出中的 s_Param_typ。这里只展示当前例子中类型编码需要的 metadata，不把类型标签检查等同于证明整个引用有效。', [M+'#L763'],['unit']),
'unit':term('unit：Sized 引用的空元数据',
 'Unit := singleton_type.\nunit : Unit.',
 '这里 unit 表示 Rust 的 ()。它是空元数据值，不是空指针、零长度，也不是目标内容；实际输出把它装箱成泛型 metadata 快照。', [M+'#L892'],[]),
'loc':term('loc(x)：变量自身的存储位置',
 'loc(x) : Addr := encode_place(x).',
 'loc(a) 是变量 a 的逻辑槽地址；loc(x) 是存放引用 x 的逻辑槽地址。x 指向 a 时 addr(x)=loc(a)，通常 loc(x) 与 loc(a) 是不同的槽。源码中的 a 是值表达式，不能在资源公式中不作说明就当成地址。', [M+'#L892'],['addr']),
'addr':term('addr(x)：引用 x 指向的地址',
 'addr(x) := (snapshot(x)).addr.',
 '先在当前状态读取引用槽的快照，再取其中的目标地址。因此 addr(x) 与 loc(x) 含义不同；前者指目标，后者指引用自身。', [M+'#L892'],['snapshot']),
'addr_at':term('addr_at(L,x)：标签状态中的引用目标',
 'addr_at(L,x) := (snapshot_at(L,x)).addr.',
 'L 是编码器保存的程序状态标签。entry/return/call_entry 是对应入口、返回、调用快照的说明性名字，实际 _before_* 等标签见输出。这个表达式固定当时引用指向哪里，不恢复目标内存当时的内容。', [E+'source/prusti-encoder/src/encoders/mir_impure.rs.html#L393'],['snapshot_at']),
'snapshot':term('snapshot(x)：从资源读取的逻辑值',
 'snapshot(x) : Snap(type_of(x)).',
 '快照不是另一个运行时堆对象。i32 使用标量快照；结构体组合字段快照；可变引用包含 addr/meta，共享引用还包含目标 val。读取快照需要相应资源权限。', [M+'#L3014',S+'#L510'],['Snap']),
'snapshot_at':term('snapshot_at(L,x)：在标签状态读取快照',
 'snapshot_at(L,x) := old[L](snapshot(x)).',
 'L 必须来自实际编码的入口、调用或 MIR 状态标签，不能凭空创建可读取的旧状态。它保存表达式取值，不产生或复制旧资源。', [],['snapshot']),
'Snap':term('Snap T：类型 T 的逻辑值表示',
 'Snap I32 := Z.\nSnap T_struct := { f : Snap U32 }.\nSnap Point := { x : Snap I32; y : Snap I32 }.',
 '此报告把整数后端快照中的载荷写成数学整数；{f:=4} 是 T 的完整字段快照，4 仅是 f 的值。Point(p,(u,v)) 中 (u,v) 是 {x:=u;y:=v} 的紧凑别名。Viper 的具体 snapshot domain、构造器与装箱函数以实际输出为准。', [M+'#L3014'],[]),
'field':term('field(p,f)：结构体字段的逻辑位置',
 'field(p,f) : Addr := typed_field_projection(p,f).',
 'p 是结构体存储地址，f 是字段名。它对应类型编码生成的字段位置函数，不直接等于 C 中 p+offset；具体 C 布局需要后端映射。不要把地址 p 写成快照 p.x。', [M+'#L752'],[]),
'elem':term('elem(base,i)：序列元素的逻辑位置',
 'elem(base,i) : Addr := typed_index_projection(base,i).',
 '只在有界下标的资源视图中使用；与具体 C 指针运算的对应关系要由类型、元素布局和后端表示证明。', [],['field']),
'Assertion':term('Assertion：资源状态上的断言',
 'Assertion := PermissionHeap -> Prop.',
 '资源断言同时描述权限和内容。** 按可兼容份额组合堆；!! 将纯命题提升为不占资源的断言；emp 是空资源。局部 assert 检查当前状态能否满足断言，不增加权限。', [],[]),
'generic':term('generic：具体资源转换为泛型资源',
 '{{Own T p v}} make_generic T p\n{{GenericOwn T p v}}.',
 '这是资源状态转换方法的前后置契约，不是 Own |-- GenericOwn 的静态蕴含。目标地址和当前快照保持，资源表示改变。', [M+'#L2924'],['Own']),
'concrete':term('concrete：泛型资源转换为具体类型资源',
 '{{GenericOwn T p v}} make_concrete T p\n{{Own T p v}}.',
 '方法根据 T 选择具体资源并保持快照。它并不凭空创建另一份目标内容；转换前的泛型权限被消费。', [M+'#L2931'],['Own']),
'fold':term('fold / unfold：命名谓词与主体的资源交换',
 '{{body_D(p)}} fold D(p) {{D(p)}}.\n{{D(p)}} unfold D(p) {{body_D(p)}}.',
 '谓词主体由类型编码定义，PCG 动作选择何时展开或收拢。这里用有前后置条件的操作表达；对 GenericOwn 不能猜一个可展开的谓词主体。', [E+'source/prusti-encoder/src/encoders/ty/use_impure.rs.html#L470'],[]),
'consume':term('consume：交出当前资源',
 '{{P}} consume P {{emp}}.',
 '对应 exhale 的资源消耗视图。对于 Weaken 到 Write，它用于清除即将覆盖的旧内容；不表示 Rust/C 内存被释放。随后赋值方法重新建立新内容的资源。', [M+'#L905'],[]),
'assign':term('assign：MIR 写入的资源契约',
 '{{emp}} assign_i32(p,n) {{Own_I32(p,n)}}\n  [encoder-selected available write target].',
 '实际赋值方法没有权限前置条件，其使用受编码器安排写目标及旧权限消耗的纪律约束；不能作为 C 中从 emp 任意分配内存的公理。C/VST 版本必须显式保留或证明可写存储。', [M+'#L3014'],[]),
'package':term('package：构造将来归还资源的 wand',
 '{{F}} package(P -* Q, script) {{P -* Q}}\n  if script transforms F ** P into Q.',
 'F 是当前可捕获的 frame。证明脚本临时假设归还左侧 P，再证明右侧 Q；这不是任意 inhale。调用关系的 Add 路径一般从被调函数后置条件获得 wand；返回处或特定循环 Add 路径才生成 package。', [],[]),
'apply_wand':term('apply_wand：使用已有归还凭证',
 '{{P ** (P -* Q)}} apply_wand {{Q}}.',
 '必须同时拥有左侧资源及相匹配的 wand。应用消耗二者并得到右侧资源；不能因图上出现 Remove 就假定 wand 必然存在。循环失败案例正显示了这个证明义务。', [],[]),
'UnblockGraph':term('UnblockGraph：按依赖安排回收步骤',
 'ready(E,e) := forall n in blocked_by(e),\n  no remaining edge in E blocks n.\nplan(E) := remove ready edges, then repeat.',
 '它对选定的 unblock 子图排序，提供 package 脚本的依赖顺序；普通回收还经过 pack/remove 的叶边、活性和 raw-pointer 保护检查。不能只凭 ready 就回收任意 PCG 边。', [E+'source/pcg/src/borrow_pcg/unblock_graph.rs.html#L48',E+'source/pcg/src/pcg/visitor/pack.rs.html#L329'],[]),
'PreMain':term('PreMain：PCG 相对于 MIR 主效果的阶段',
 'AnalysisLocation := (MIR Location, EvalStmtPhase).\nMIR Location := (BasicBlock, StatementIndex).',
 'bb0[6] 是 MIR 位置；PreMain 是 PCG 在该位置执行主效果前的阶段。它不是新的 MIR 基本块、不是 Rust 标注语法。写入 *x 的主效果之前，需要先按 PCG 访问需求暴露目标资源。', [E+'source/prusti-encoder/src/encoders/mir_impure.rs.html#L2117'],['AnalysisLocation']),
'EX':term('EX：存在量化的资源断言','EX v, P(v) := fun h => exists v, P(v)(h).','表示持有某个当前值 v 的资源；不要求在图中找到这个数值。',[],[]),
'ALL':term('ALL：全称量化的断言','ALL v, P(v) := fun h => forall v, P(v)(h).','在 W 中对将来归还时的值 z 量化；归还资源提供哪个当前值，就按该值实例化恢复关系。',[],[]),
'SEP':term('SEP：有限分离合取','SEP i in [0,length vs), P(i) := separating_fold(P).','只遍历合法的有限下标；Frame 排除正在写入的下标，保留其他元素资源。',[],[]),
'emp':term('emp：空资源断言','emp h := h = empty_heap.','不占有目标字段，也不自动提供 C 存储有效性。',[],[]),
'Ty_INV':term('Ty_INV：后端表示不变式','Ty_INV T : Snap T -> Prop.','它由具体类型表示决定，不等于 Rust 引用的完整有效性要求，也不是自动推导的任意用户类型不变式。此模型中整数表示的不变式为 True，不据此声称完整 Rust/C 整数语义。',[],[]),
}
ALIASES={'close_index':'open_index','make_generic':'generic','make_concrete':'concrete','unfold':'fold','meta_type':'Metadata','i32_val':'cell','u32_val':'cell','ref_addr':'DirectMut','ref_meta':'DirectMut','shared_addr':'RefShared','shared_meta':'RefShared','unit':'unit','Ty_INV_I32':'Ty_INV','Ty_INV_T':'Ty_INV','Ty_INV_Slice':'Ty_INV'}

CASE_CONTEXT={
'borrow-create':'loc(a) 是整数槽，loc(x) 是引用槽；p 泛指目标地址，s 泛指引用槽地址。Ref(s,p) 特化 T=I32、metadata=unit。I/G 中的 v 是整数当前值；类型换成 i64 时需选择 I64 的值字段及对应转换方法。',
'deref':'q=addr(x)，v 为转换前的当前整数值。赋值尚未执行，不能把它提前写成 4。',
'weaken':'q=addr(x)，old_value 是写入前的快照。W 是 PCG Write 能力；赋值纪律确保此时旧值资源已交出。',
'borrow-end':'q=addr(x)=loc(a)；两个地址表达式来自同一次借用建立的关系。恢复的是当前值 4。',
'field-open':'q=addr(x)。T 是官方例子的单字段结构体，s={f:=v} 是结构体快照，v 是字段 u32 快照。',
'field-close':'q=addr(x)。fold 把字段资源重新组成 T，generic 再转换表示；剩余 Borrow 回收后才能读取 a.f。',
'reborrow-chain':'q=addr(z)=addr(y)=addr(x)=loc(a)。这些表达式表示同一个目标，只有一份目标资源。close 和 return_to 是 generic/concrete 的脚本别名。',
'basic-wand':'p 固定入口 num 的目标，q 固定返回 result 的目标；R 是存在量化的目标资源。函数头 requires/ensures 与函数内的局部 assert 含义不同。',
'call-edge':'argument 指调用点传入的 &mut pt；p 是输入目标，q 是返回 y 的目标；y0 为调用前 pt.y。Point 快照 (u,v) 是 {x:=u;y:=v} 的缩写。',
'package':'p=addr_at(entry,pt)，q=addr_at(return,result)，y0 是入口 pt.y 的值；z 是未来归还时的 x 值。Point 快照 (u,v) 表示 {x:=u;y:=v}，W 的 x/z、y/y0 关系由用户 after_expiry 提供。',
'apply':'p 是原 Point 目标，q=addr(y)，y0 是调用前 pt.y=72；W 来源于调用的后置条件。Point 快照 (u,v) 表示 {x:=u;y:=v}。',
'history':'p 是旧引用快照中的目标；G_Point/R_Point 隐藏当前字段快照。地址固定不意味着旧字段值复原。',
'shared':'程序中的 pt/result 是引用槽；s=snapshot(pt)、r=snapshot(result) 是引用快照，包含 addr/meta/val。公式 Shared(T,slot,snapshot) 的第 2 个参数总是槽地址。GenericRead 的份额 rho_read 不是生命周期 rho。',
'projection':'cs=snapshot(c)，cs.x 是引用字段的快照；rho 是该引用的 region。Indirect 选择属于 rho 的目标资源，不是 c 自身的直接字段资源。',
'flow':'P 是该动作前已经成立的资源断言；FUTURE 与 BorrowFlow 是分析标签/关系，不能据此定义新堆对象。',
'unsize':'base 是数组/切片的元素基位置；vs 是当前元素序列；n=3。Seq/Slice 是规约序列视图，仍需后端表示关系支持。',
'index':'base 和 vs 来自此前切片视图；目标下标 k=1，合法域为 [0,length vs)。Frame 保留除 k 外的资源。',
'unsize-undo':'base 不变；原下标 1 的值 37 已被写成 3，undo 后序列仍为 [13;3;72]。',
'branch':'r 在两条互斥执行路径上指向 a 或 b。guard_a/guard_b 由 MIR 路径标志确定，未被借出的另一变量在 frame 中。',
'loop-invariant':'loc(i) 是循环变量槽，n 是切点当前值。切点在循环体用户 body_invariant 所在处；资源项来自活性/初始化/能力分析。',
'loop-wand':'x 是长期来源，r 是短期 blocker，L 是实际循环边界标签。I_loop/W 是生成的义务，实跑未证明恢复成立。',
'remote':'List 是 Rust 类型；ListRes 是资源谓词；p_entry 是入口 self 的目标，q 是 outer_package 标签处 current 的目标。实际运行失败。',
'inhale-path':'P 表示 panic 分支带路径条件的已有状态；必须先证明 False 再截断路径。这里只描述失败分支，不是在正常路径制造资源。',
}

def target(name,context=None):
    key=ALIASES.get(name,name)
    if key in TERMS:return 'term-'+key
    if context and name in LOCAL_NAMES:return 'symbols-'+context
    return None

LOCAL_NAMES=set('I G R U Ref OwnInt Own_T R_T Point G_Point Own_Point R_Point W Direct Indirect P SharedPoint SharedI32 Read Seq Slice Frame I_resource I_body I_loop ListRes W_remote ready close return_to guarded guard_a guard_b open_index close_index undo_unsize cast_array_to_slice entry_value Ty_INV_Slice'.split())

def dependencies(ann,defs):
    names=set(re.findall(r'\b[A-Za-z_]\w*\b',ann+'\n'+defs))
    seen=set();todo=[ALIASES.get(n,n) for n in names if ALIASES.get(n,n) in TERMS]
    while todo:
        key=todo.pop()
        if key in seen:continue
        seen.add(key);todo.extend(TERMS[key]['dependencies'])
    return sorted(seen)

def render_templates(cases,code):
    esc=html.escape;out=[]
    for key,item in TERMS.items():
        body='<h2>'+esc(item['title'])+'</h2>'+code(item['definition'],'definitions')+'<p>'+esc(item['meaning'])+'</p>'
        if item['dependencies']:
            body+='<p>相关定义：'+' · '.join('<a class="term-link" data-term="term-'+n+'" href="#term-'+n+'">'+esc(n)+'</a>' for n in item['dependencies'])+'</p>'
        body+='<nav>'+''.join('<a href="'+url+'" target="_blank" rel="noopener">实跑依据 '+str(i)+'</a>' for i,url in enumerate(item['refs'],1))+'</nav>'
        out.append('<template id="term-'+key+'">'+body+'</template>')
    for ident,(ann,defs,summary) in cases.items():
        body='<h2>本页符号与参数</h2><p>'+esc(CASE_CONTEXT[ident])+'</p>'+code(defs,'definitions')
        for key in dependencies(ann,defs):
            item=TERMS[key]
            body+='<details><summary>'+esc(item['title'])+'</summary>'+code(item['definition'],'definitions')+'<p>'+esc(item['meaning'])+'</p></details>'
        out.append('<template id="symbols-'+ident+'">'+body+'</template>')
    return ''.join(out)
