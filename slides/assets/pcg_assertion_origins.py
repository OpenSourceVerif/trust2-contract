"""Per-assertion provenance for the compact presentation, tied to frozen evidence.

These explanations describe the displayed resource abstraction. They do not
claim that Prusti emits the presentation's assert statements verbatim.
"""
import html

def origin(assertion, source, reason, refs, detail):
    return dict(assertion=assertion, source=source, reason=reason, refs=refs, detail=detail)

ORIGINS = {'borrow-create': [{'assertion': '//@ assert I(loc(a),6)',
                    'source': 'MIR赋值·类型',
                    'reason': '初始化建立 i32 资源，值为 6。',
                    'refs': ['viper:880', 'viper:3014'],
                    'detail': 'MIR _1 = const 6_i32 编码为 p_Int_i32_assign；其后置条件给出整数谓词权限和 snapshot = 6。PCG '
                              '借用图不提供常量 6。'},
                   {'assertion': '//@ proof make_generic I32 loc(a)',
                    'source': 'MIR引用编码',
                    'reason': 'post_assign_folds 转换目标资源。',
                    'refs': ['encoder-ref', 'viper:892'],
                    'detail': '可变引用右值构造引用快照，并注册 post_assign_folds。赋值后按目标类型调用 make_generic_Int_i32；本点 '
                              'phase_actions 为空，不能归因于一条 Add Borrow 动作。'},
                   {'assertion': '//@ assert Ref(loc(x),loc(a)) ** G(loc(a),6)',
                    'source': '引用赋值·转换契约',
                    'reason': '建立 Ref，并保值转换 I → G。',
                    'refs': ['viper:892', 'viper:2924', 'pcg'],
                    'detail': '引用赋值契约建立 x 自身的地址/metadata 资源；generic 转换得到 a 的泛型目标资源并保持快照 6。PCG 记录 a 被 x '
                              '的借用阻塞；Ref 与 G 描述不同资源，并非复制目标所有权。'}],
 'deref': [{'assertion': '//@ assert G(q,v)',
            'source': '已有资源·引用快照',
            'reason': '沿用借用后的 G；快照确定 q。',
            'refs': ['viper', 'rust'],
            'detail': 'G 来自此前引用赋值后的 generic 转换；q 来自引用地址快照。v 表示当前内容，图不负责枚举它的数值。'},
           {'assertion': '//@ assert I(q,v)',
            'source': 'PCG Deref Add·类型',
            'reason': '触发 concrete，保持同一值 v。',
            'refs': ['pcg', 'viper'],
            'detail': '访问 *x 所需的 Deref Add 经类型分派生成 make_concrete_Int_i32；转换方法的契约给出具体 i32 资源并保持目标快照。'}],
 'weaken': [{'assertion': '//@ assert I(q,old_value)',
             'source': '类型转换契约',
             'reason': '此前 concrete 已暴露旧值资源。',
             'refs': ['viper'],
             'detail': '写入前已完成 Deref 展开。old_value 是当前快照的逻辑名字，不是 PCG 常量；Weaken 接下来消耗这份旧资源。'},
            {'assertion': '//@ assert I(q,4)',
             'source': 'MIR赋值·方法契约',
             'reason': '写入常量 4，建立新的值资源。',
             'refs': ['viper', 'pcg'],
             'detail': 'PCG Weaken(...,W) 负责旧资源消耗的调度；新资源与值 4 来自 MIR 写入及 p_Int_i32_assign 的后置条件。'}],
 'borrow-end': [{'assertion': '//@ assert I(q,4) ** !!(q = loc(a))',
                 'source': 'MIR写入·引用地址',
                 'reason': '*x 写入 4，x 仍指向 a。',
                 'refs': ['viper', 'rust'],
                 'detail': '数值来自 *x = 4 的赋值契约；地址等式来自建立引用时的快照，而非借用结束时恢复旧内容。'},
                {'assertion': '//@ assert I(loc(a),4)',
                 'source': 'PCG回收·转换契约',
                 'reason': '移除 Deref/Borrow，保值恢复 a。',
                 'refs': ['pcg', 'viper'],
                 'detail': '叶边回收顺序触发 make_generic 后 make_concrete，普通 mutable Borrow 的移除恢复 assigned_ref '
                           '的目标。两次转换保持当前值 4。'},
                {'assertion': 'assert!(a == 4);',
                 'source': '用户断言·当前快照',
                 'reason': '从恢复后的 I(a,4) 检查值等式。',
                 'refs': ['rust', 'viper'],
                 'detail': 'assert! 原本就在 Rust 测试中；Prusti 将其作为待验证的程序条件。此前写入和资源恢复共同提供证明所需的值事实。'}],
 'field-open': [{'assertion': '//@ assert R_T(q)',
                 'source': '类型 T·已有资源',
                 'reason': '引用展开后持有结构体资源。',
                 'refs': ['viper', 'rust'],
                 'detail': 'T 的字段定义提供 Own_T 的结构；目标地址来自 x 的快照。存在量化隐藏本例尚不需要使用的字段具体值。'},
                {'assertion': '//@ assert EX v, U(field(q,f),v)',
                 'source': 'PCG字段展开·类型',
                 'reason': '展开 Own_T，暴露 u32 字段 f。',
                 'refs': ['pcg', 'viper'],
                 'detail': 'Place expansion 决定展开时机；类型谓词体决定展开后得到哪个字段的资源。此例 T 只有 f:u32。'}],
 'field-close': [{'assertion': '//@ assert U(field(q,f),4)',
                  'source': 'MIR字段赋值',
                  'reason': 'x.f = 4 建立字段当前值。',
                  'refs': ['viper', 'rust'],
                  'detail': '字段资源来自类型编码；数值 4 由 MIR 赋值方法的后置条件建立。'},
                 {'assertion': '//@ assert GenericOwn T q {f:=4}',
                  'source': 'PCG收拢·类型契约',
                  'reason': 'fold 后 generic，保持字段值 4。',
                  'refs': ['pcg', 'viper'],
                  'detail': '投影收拢重组结构体谓词，随后 Deref Remove 将其转换为泛型目标资源；不会恢复初始化时的 6。 T 的快照明确写作 {f:=4}；整数 4 '
                            '仅为字段快照。'},
                 {'assertion': 'assert!(a.f == 4);',
                  'source': '用户断言·回收结果',
                  'reason': '后续借用回收后读取字段值 4。',
                  'refs': ['rust', 'viper'],
                  'detail': 'Rust 原断言检查字段值。脚本在读取 a.f 之前列出 Borrow Remove 对应的 concrete，恢复目标的具体类型资源，再验证当前字段值。'}],
 'reborrow-chain': [{'assertion': '//@ assert I(q,4) ** !!(q = loc(a))',
                     'source': 'MIR写入·重借用地址',
                     'reason': 'z 写入 4，各层引用指向同一目标。',
                     'refs': ['rust', 'viper'],
                     'detail': '连续 &mut 重借用建立目标地址关系，*z = 4 建立当前值。PCG 记录这些借用依赖，不提供数值 4。'},
                    {'assertion': '//@ assert I(loc(a),4)',
                     'source': 'PCG依赖·保值转换',
                     'reason': '依次回收 z、y、x，恢复 a 的资源。',
                     'refs': ['pcg', 'viper'],
                     'detail': '普通回收经过 pack/remove 状态更新；各层 generic/concrete 契约保持同一目标当前值。UnblockGraph 另用于 '
                               'package 脚本计划。'},
                    {'assertion': 'assert!(a == 4);',
                     'source': '用户断言·当前快照',
                     'reason': '读取恢复后的 a，验证写入结果。',
                     'refs': ['rust', 'viper'],
                     'detail': '该断言是原 Rust 测试条件；PCG 恢复访问能力，赋值与快照事实支持值等式。'}],
 'basic-wand': [{'assertion': '//@ requires R(p)',
                 'source': '函数签名·类型',
                 'reason': '输入 &mut i32 要求目标资源。',
                 'refs': ['rust', 'viper'],
                 'detail': '参数类型决定间接目标资源谓词，入口引用快照确定 p。这里只展示函数的间接资源契约，直接引用槽另行处理。'},
                {'assertion': '//@ ensures R(q) ** (R(q) -* R(p))',
                 'source': 'PCG shape·返回契约',
                 'reason': '输出资源及将来归还输入的 wand。',
                 'refs': ['pcg', 'viper', 'rust'],
                 'detail': "签名 'l: 's 和借用形状关联输入/输出生命周期投影；类型编码提供两侧资源。返回处以 return_unblock_plan 构造 package "
                           '义务，生成资源与归还 wand；不从中推导额外数值关系。'}],
 'call-edge': [{'assertion': '//@ assert P(p)',
                'source': '调用前状态·参数类型',
                'reason': '将已有 Point 资源用于调用前置。',
                'refs': ['rust', 'viper'],
                'detail': '调用者已有的 Point 目标资源满足被调方法资源前置条件；p 由调用入口参数快照实例化。'},
               {'assertion': '//@ assert R(q) ** W(q,p,y0)',
                'source': '方法后置·用户规约',
                'reason': '得到输出与 wand；值关系来自 pledge。',
                'refs': ['viper', 'rust', 'pcg'],
                'detail': '返回资源与 wand 由 get_mut_x 的后置条件提供。PCG coupled Add 记录调用依赖，该 Add 编码分支不另行 package。W 内的 '
                          'x/z、y/y0 等式来自用户 after_expiry。'}],
 'package': [{'assertion': '//@ at return: package W(q,p,y0) {',
              'source': 'PCG计划·用户规约',
              'reason': '按回收依赖构造含到期值条件的 wand。',
              'refs': ['pcg', 'viper', 'rust'],
              'detail': 'UnblockGraph 产生恢复输入的有序计划；返回借用的资源形状和用户 after_expiry 一起决定 W 的义务。package 捕获未借出的字段 '
                        'frame。'},
             {'assertion': '//@   assume returned G(q,z)',
              'source': 'wand引入规则',
              'reason': '在 package 内假设将来归还的左侧。',
              'refs': ['viper'],
              'detail': '这是证明 wand 时的局部假设；z 表示归还时内容。它不授权调用者在正常执行路径任意增加资源。'},
             {'assertion': '//@   check (pt.x = z /\\ pt.y = y0)',
              'source': '用户 after_expiry',
              'reason': '检查归还的 x 与保留的 y 值关系。',
              'refs': ['rust', 'viper'],
              'detail': 'x = z 来自归还的字段资源，y = y0 来自捕获并保留的未借字段。这个值性质由用户声明，PCG 只提供恢复依赖和顺序。'}],
 'apply': [{'assertion': '//@ assert G(q,72) ** W(q,p,y0)',
            'source': 'MIR写入·调用后置',
            'reason': '72 来自写入；W 来自先前调用。',
            'refs': ['viper', 'rust'],
            'detail': '*y = 72 及随后的 generic 转换提供 G(q,72)；已有 W 来自 get_mut_x 的方法后置条件，不在此处重新构造。'},
           {'assertion': '//@ assert Point(p,(72,y0))',
            'source': 'PCG coupled Remove',
            'reason': 'apply W 后 concrete，恢复 Point。',
            'refs': ['pcg', 'viper'],
            'detail': '调用借用依赖解除时，Prusti 调用 call_wands 并 apply；以当前值 72 实例化到期资源与用户关系，再恢复具体 Point 资源。'},
           {'assertion': 'assert!(pt.x == pt.y);',
            'source': '用户断言·初始化',
            'reason': '已恢复 x = 72；初始化给出 y0 = 72。',
            'refs': ['rust', 'viper'],
            'detail': '仅有资源 wand 不能推出两字段相等；还需要初始化 pt.y = 72，以及 after_expiry 保持 pt.y 的事实。'}],
 'history': [{'assertion': '//@ assert G_Point(p)',
              'source': '回收结果·PCG历史节点',
              'reason': '恢复资源定位到旧引用的目标 p。',
              'refs': ['pcg', 'viper'],
              'detail': '前面的 wand apply 已提供目标资源；历史标签固定被移动引用原来指向哪里，并不凭标签产生资源。'},
             {'assertion': '//@ assert R_Point(p)',
              'source': '类型转换契约',
              'reason': '对同一历史目标执行 concrete。',
              'refs': ['viper'],
              'detail': 'make_concrete Point 的契约将已有泛型资源转为具体 Point 资源，保留当前目标内容；old 只固定地址身份。'},
             {'assertion': 'assert!(pt.x == pt.y);',
              'source': '用户断言·写入及 pledge',
              'reason': '使用当前值 72 和保存的 y 值。',
              'refs': ['rust', 'viper'],
              'detail': '历史地址本身不蕴含字段相等。值关系来自调用前初始化、*y = 72 及 after_expiry，经恢复后用于验证原 Rust 断言。'}],
 'shared': [{'assertion': '//@ assert Shared(Point,loc(pt),s)',
             'source': '共享参数·类型编码',
             'reason': '输入快照关联 Point 的读资源。',
             'refs': ['viper', 'rust'],
             'detail': '共享引用表示由引用槽的地址/metadata 权限与目标 GenericRead 组成；后者是 p_Param 的读权限及目标快照约束。Shared '
                       '不再被直接等同于展开后的具体字段 cell。'},
            {'assertion': '//@ assert Shared(I32,loc(result),r)\n//@       ** !!(r.val = s.val.x)',
             'source': '共享字段编码·用户 ensures',
             'reason': '传播读视图，并检查结果值等式。',
             'refs': ['viper', 'rust', 'pcg'],
             'detail': 'MIR &pt.x 构造返回引用；共享目标仍通过 p_Param 的读权限表示。r 是结果引用快照，s 是输入引用快照，r.val = s.val.x 来自用户 '
                       'ensures。具体字段分离视图需要额外转换规则。'}],
 'projection': [{'assertion': '//@ assert Indirect(cs,rho)',
                 'source': '类型·region匹配',
                 'reason': '选择 c 中属于 rho 的目标资源。',
                 'refs': ['viper', 'rust'],
                 'detail': 'Indirect 的定义按类型中的 region 出现位置选择引用目标资源；它描述已有资源，不因投影节点而分配新对象。'},
                {'assertion': '//@ assert Indirect(cs,rho)',
                 'source': 'PCG投影展开·skip',
                 'reason': '仅更新图；该分支保持原断言。',
                 'refs': ['pcg', 'viper'],
                 'detail': '当前 lifetime projection expansion 的编码分支不直接 '
                           'fold/unfold，故局部资源状态保持。不能将这结论推广到同语句的其他动作。'}],
 'flow': [{'assertion': '//@ assert P',
           'source': '进入动作前的已有状态',
           'reason': 'P 为当前资源断言，无新增假设。',
           'refs': ['viper'],
           'detail': 'P 是用于描述局部效果的任意已有资源断言；不是根据 FUTURE 节点新合成的一份目标资源。'},
          {'assertion': '//@ assert P',
           'source': 'PCG BorrowFlow·skip',
           'reason': '普通流边只记依赖，资源状态不变。',
           'refs': ['pcg', 'viper'],
           'detail': '本例普通 BorrowFlow/FUTURE 在编码中只产生分析信息/注释，因此可用 skip 的保状态规则描述；Unsize 不属于该分支。'}],
 'unsize': [{'assertion': '//@ assert Seq(base,[13;37;72])',
             'source': 'MIR初始化·数组类型',
             'reason': '初值给出元素，类型给出长度 3。',
             'refs': ['rust', 'viper'],
             'detail': '序列常量来自原始数组初始化，长度来自数组类型。Seq 是对后端数组资源的规约视图，不是 PCG 中的数值列表。'},
            {'assertion': '//@ assert Slice(base,3,[13;37;72])',
             'source': 'MIR Unsize·转换契约',
             'reason': '建立切片视图，保留地址与元素值。',
             'refs': ['viper', 'pcg'],
             'detail': 'MIR cast 执行数组到切片转换；PCG BorrowFlow 保存源、目标、Unsize 元数据以支持后续 undo。Slice '
                       '的序列表示关系属于规约视图接口。'}],
 'index': [{'assertion': '//@ assert Slice(base,3,vs) ** !!(0 <= 1 < 3)',
            'source': '切片资源·MIR边界检查',
            'reason': '已有长度 3；下标 1 满足边界。',
            'refs': ['viper', 'rust'],
            'detail': 'Slice 来自此前 Unsize；纯断言使用实际下标、长度及 MIR 检查，不从索引 expansion 边直接推导。'},
           {'assertion': '//@ assert I(elem(base,1),3) ** Frame(base,vs,1)',
            'source': 'PCG索引展开·MIR写入',
            'reason': '分离目标后写入 3，其余元素保留。',
            'refs': ['pcg', 'viper'],
            'detail': '索引展开提供目标元素访问；赋值契约建立该元素新值 3。Frame 表示未写元素的资源；序列资源抽象由表示接口连接到实际编码。'}],
 'unsize-undo': [{'assertion': '//@ assert Slice(base,3,[13;3;72])',
                  'source': 'MIR元素赋值·切片视图',
                  'reason': '只把第 1 项改为 3，保留其余值。',
                  'refs': ['rust', 'viper'],
                  'detail': '当前序列由初始数组与 y[1] = 3 的赋值效果组合得到。PCG 负责依赖，不提供元素数值更新函数。'},
                 {'assertion': '//@ assert Seq(base,[13;3;72])',
                  'source': 'PCG Unsize Remove·undo',
                  'reason': '恢复数组视图，保留修改后内容。',
                  'refs': ['pcg', 'viper'],
                  'detail': '移除带 Unsize 元数据的 BorrowFlow 时，Prusti 调用 undo。源目标与转换信息用于恢复原数组视图，当前值不回滚。'}],
 'branch': [{'assertion': '//@ assert I(addr(r),9)',
             'source': 'MIR赋值·引用快照',
             'reason': '*r 写入 9；快照确定实际目标。',
             'refs': ['rust', 'viper'],
             'detail': '值 9 来自赋值契约；r 在所选分支指向 a 或 b，其地址来自分支中的引用赋值。'},
            {'assertion': '//@ assert guarded guard_a I(loc(a),9) **\n//@      guarded guard_b I(loc(b),9)',
             'source': 'PCG路径条件·回收契约',
             'reason': '按实际分支恢复相应目标的值资源。',
             'refs': ['pcg', 'viper'],
             'detail': '边的路径条件变成恢复语句的 guard；保值转换将当前目标资源交回所选变量。另一未借变量仍在 frame 中，不凭空复制资源。'}],
 'loop-invariant': [{'assertion': 'body_invariant!(i > 2);',
                     'source': '用户循环规约',
                     'reason': '原程序要求每次循环体切点满足 i > 2。',
                     'refs': ['rust', 'viper'],
                     'detail': '此表达式由用户提供；PCG 不自动发现 n > 2 这一数值不变式。'},
                    {'assertion': '//@ invariant EX n, I(loc(i),n) ** !!(n > 2)',
                     'source': 'PCG活性/能力·用户规约',
                     'reason': '组合资源不变式与用户值条件。',
                     'refs': ['pcg', 'viper', 'rust'],
                     'detail': 'used/live/initialized/capability 等分析决定切点需要保有的资源；类型编码给出 I，用户条件给出 n > '
                               '2。进入/再次进入该循环体切点均需检查。'}],
 'loop-wand': [{'assertion': '//@ Add + edge_to_loop: package W(L)',
                'source': 'PCG LoopAbstraction',
                'reason': '入口/回边生成归还义务，实跑失败。',
                'refs': ['pcg', 'viper', 'run'],
                'detail': 'successor.actions 上指向循环的 Add 触发 package；类型与标记状态提供 wand 两侧。该运行最终报 '
                          'wand.not.found，不把生成的义务当作已证明断言。'},
               {'assertion': '//@ Remove Loop: apply_wand W(L)',
                'source': 'PCG Loop Remove',
                'reason': '要求可用的匹配 wand；本次未找到。',
                'refs': ['pcg', 'viper', 'run'],
                'detail': 'Remove 路径调用 apply。实际验证失败意味着不能声称退出后目标资源已成功恢复；必须保留失败结论。'}],
 'remote': [{'assertion': '//@ package (ListRes(q) -* ListRes(p_entry))',
             'source': 'PCG Remote·循环抽象',
             'reason': '以入口目标构造归还义务，尚未证明。',
             'refs': ['pcg', 'viper', 'run'],
             'detail': 'Remote 基底按入口 old 解释，List 类型提供抽象资源，循环计划决定恢复脚本。此实跑报 '
                       'insufficient.permission；不能据此声称获得已验证的 List 归还契约。'}],
 'inhale-path': [{'assertion': 'assert!(a == 4);',
                  'source': '用户 Rust 断言',
                  'reason': '该条件为假时，MIR 才进入 panic 分支。',
                  'refs': ['rust', 'viper'],
                  'detail': '源程序 assert! 生成条件分支；下方注释描述失败分支上的不可达证明义务，而不是正常执行中的资源获取。'},
                 {'assertion': '//@ assert P',
                  'source': 'MIR失败分支状态',
                  'reason': 'P 包含路径条件及此前已有事实。',
                  'refs': ['viper'],
                  'detail': 'P 表示该失败分支入口的验证状态；它不是任意一个能无条件推出 False 的可达状态。'},
                 {'assertion': '//@ proof check False; assume False',
                  'source': 'panic控制流编码',
                  'reason': '先证明不可达，再截断这条路径。',
                  'refs': ['viper'],
                  'detail': '实际输出先 exhale false，再 inhale false。必须先由路径条件与已知值事实排除失败路径，不能用 assume 任意修复证明失败。'},
                 {'assertion': '//@ assert !!False',
                  'source': '已检查的不可达条件',
                  'reason': '表示该分支不可达，不是获得新资源。',
                  'refs': ['viper'],
                  'detail': '这个断言概括通过 False 检查后的路径状态；其依据是已验证的不可达义务，PCG AddEdge 不参与产生该条件。'}]}


def resolve_refs(item, case):
    """Resolve symbolic references exclusively within the portable slide bundle."""
    urls=[]
    for ref in item['refs']:
        if ref.startswith('viper'):
            line=ref.split(':')[1] if ':' in ref else case['viper_start_line']
            url=case['viper_file']+'.html#L'+str(line)
        elif ref=='rust':
            url=f"evidence/pcg-encoding-20261125/{case['case']}/{case['case']}.rs.html#L{case['source_start_line']}"
        elif ref=='pcg':url=case['point']
        elif ref=='run':url=f"evidence/pcg-encoding-20261125/{case['case']}/run.log"
        elif ref=='encoder-ref':url='evidence/pcg-encoding-20261125/source/prusti-encoder/src/encoders/mir_impure.rs.html#L393'
        else:raise ValueError(ref)
        urls.append(url)
    return urls


def locate(ident, annotation):
    lines=annotation.splitlines();cursor=0;items=[]
    for item in ORIGINS[ident]:
        wanted=item['assertion'].splitlines()
        for start in range(cursor,len(lines)-len(wanted)+1):
            if all(lines[start+i].strip().startswith(s.strip()) for i,s in enumerate(wanted)):
                end=start+len(wanted)-1
                items.append(dict(item,start_line=start+1,end_line=end+1))
                cursor=end+1;break
        else:raise ValueError(f'Unmatched provenance {ident}: {item["assertion"]}')
    return items


def render_annotation(ident, annotation, case, highlight):
    items=locate(ident,annotation);ends={item['end_line']:item for item in items}
    out=[]
    for number,line in enumerate(annotation.splitlines(),1):
        out.append('<span class="annotation-line">'+highlight(line or ' ')+'</span>')
        if number in ends:
            item=ends[number];url=resolve_refs(item,case)[0]
            text='// '+item['source']+' → '+item['reason']
            out.append('<span class="origin-line"><a href="'+html.escape(url,quote=True)+'" target="_blank" rel="noopener" title="查看对应实跑输出或源码">'+html.escape(text)+'</a></span>')
    return '<pre class="annotated"><code>'+''.join(out)+'</code></pre>'


def render_details(ident, annotation, case, highlight):
    out=['<section class="assertion-origins"><h3>逐条断言：来源与生成理由</h3>']
    for item in locate(ident,annotation):
        out.append('<div class="origin-record"><pre><code>'+highlight(item['assertion'])+'</code></pre><p><strong>'+html.escape(item['source'])+'</strong>：'+html.escape(item['detail'])+'</p><nav>')
        for i,url in enumerate(resolve_refs(item,case),1):
            out.append('<a href="'+html.escape(url,quote=True)+'" target="_blank" rel="noopener">证据 '+str(i)+'</a>')
        out.append('</nav></div>')
    return ''.join(out)+'</section>'
