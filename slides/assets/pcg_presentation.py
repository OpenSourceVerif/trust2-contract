"""Compact, offline presentation view for the evidence-backed rule report."""
import html,json,re,textwrap
from pcg_assertion_origins import locate, resolve_refs, render_annotation, render_details
from pcg_terms import target as term_target, render_templates, dependencies, CASE_CONTEXT
from pcg_rule_focus import FOCUS, presentation_parts

# Preserve complete aliases for symbol dialogs. Each visible card focuses on
# one resource rule; the source and run artifacts are never rewritten.
COMPACT = {
'borrow-create': (
'let mut a = 6;\n//@ assert I(loc(a),6)\nlet x = &mut a;\n//@ proof make_generic I32 loc(a)\n//@ assert Ref(loc(x),loc(a)) ** G(loc(a),6)',
'I(p,v) := cell 1 p i32_val v\nG(p,v) := GenericOwn I32 p v\nRef(s,p) := DirectMut I32 s (p,unit)\nDirectMut T s (p,m) :=\n  cell 1 s ref_addr p ** cell 1 s ref_meta m\n  ** !!(meta_type(m) = Metadata(T))',
'MIR 赋值及类型契约建立资源与值；PCG 记录借用阻塞，供后续访问和回收使用。'),
'deref': (
'//@ assert G(q,v)\n//@ proof make_concrete I32 q\n//@ assert I(q,v)\n*x = 4;',
'q := addr(x)\nI(q,v) := cell 1 q i32_val v\nG(q,v) := GenericOwn I32 q v\n{{G(q,v)}} concrete q {{I(q,v)}}\nconcrete 是有契约的操作，不是逻辑蕴含。',
'Deref Add 根据 *x 的访问需求，在赋值前展开目标资源；值保持不变。'),
'weaken': (
'//@ assert I(q,old_value)\n//@ proof consume I(q,old_value)\n*x = 4;  // MIR 写入\n//@ assert I(q,4)',
'q := addr(x)\nI(q,v) := cell 1 q i32_val v\nconsume : {{I(q,v)}} consume {{emp}}\nassign : {{emp}} write q 4 {{I(q,4)}}\n仅为已分配、可写目标的编码契约；不是 C 分配。',
'Weaken(..., W) 调度旧资源的交出；新值 4 来自 MIR 赋值。'),
'borrow-end': (
'*x = 4;\n//@ assert I(q,4) ** !!(q = loc(a))\n//@ proof generic q;   // Remove Deref\n//@       concrete q;  // Remove Borrow\n//@ assert I(loc(a),4)\nassert!(a == 4);',
'I(p,v) := cell 1 p i32_val v\nG(p,v) := GenericOwn I32 p v\n{{I(q,v)}} generic q {{G(q,v)}}\n{{G(q,v)}} concrete q {{I(q,v)}}\nq := addr(x)；借用时 q = loc(a)。',
'叶边回收在 bb0[7] 恢复 a；不要求源程序出现 drop(x)。'),
'field-open': (
'//@ assert R_T(q)\n//@ proof unfold Own_T\n//@ assert EX v, U(field(q,f),v)\nx.f = 4;',
'q := addr(x)\nU(p,v) := cell 1 p u32_val v\nOwn_T(q,s) := U(field(q,f),s.f)\nR_T(q) := EX s, Own_T(q,s)\nfield(q,f)：对象 q 中字段 f 的逻辑地址。\ns.f：快照 s 中字段 f 的值；本例 s={f:=v}。',
'Place expansion 在字段访问前展开结构体谓词，暴露 f 的资源。'),
'field-close': (
'x.f = 4;\n//@ assert U(field(q,f),4)\n//@ proof fold Own_T; generic T q\n//@ assert GenericOwn T q {f:=4}\n//@ proof concrete T q  // Borrow Remove\nassert!(a.f == 4);',
'q := addr(x)\nU(p,v) := cell 1 p u32_val v\nOwn_T(q,s) := U(field(q,f),s.f)\n{{U(field(q,f),v)}} fold Own_T\n  {{Own_T(q,{f:=v})}}\n再由 generic 转成 GenericOwn T q {f:=v}。',
'投影收拢先 fold T，再处理 Deref Remove；字段当前值仍为 4。'),
'reborrow-chain': (
'*z = 4;\n//@ assert I(q,4) ** !!(q = loc(a))\n//@ proof close z; return_to y\n//@       close y; return_to x\n//@       close x; return_to a\n//@ assert I(loc(a),4)\nassert!(a == 4);',
'q := addr(z)\nI(p,v) := cell 1 p i32_val v\nclose := make_generic I32\nreturn_to := make_concrete I32\nready(e) := 每个 blocker 均不被剩余边阻塞\n按依赖回收；package 另查询 UnblockGraph。',
'按 blocker 依赖先回收 z、再 y、再 x；三条引用共享一份目标资源。'),
'basic-wand': (
"//@ requires R(p)\n//@ ensures R(q) ** (R(q) -* R(p))\nfn basic<'l, 's>(num: &'l mut i32) -> &'s mut i32\nwhere 'l: 's {\n    &mut *num\n    //@ proof package (R(q) -* R(p))\n    //@   using return_unblock_plan\n}",
'R(p) := EX v, GenericOwn I32 p v\np := addr_at(entry,num)\nq := addr_at(return,result)\nW(q,p) := R(q) -* R(p)\npackage：以归还 R(q) 为假设证明 R(p)。',
"'l: 's 形成输入→输出依赖；返回处生成输出资源及反向归还 wand。"),
'call-edge': (
'//@ assert P(p)\nlet y = get_mut_x(&mut pt);\n//@ assert R(q) ** W(q,p,y0)\n//@ Add coupled edge：此处不另行 package',
'p := addr_at(call_entry,argument)\nq := addr(y)；y0 := entry_value(pt.y)\nG(q,z) := GenericOwn I32 q z\nR(q) := EX z, G(q,z)\nP(p) := EX s, G_Point(p,s)\nG_Point(p,s) := GenericOwn Point p s\nW(q,p,y0) := ALL z, G(q,z) -* G_Point(p,(z,y0))',
'coupled Add 记录调用依赖；返回资源和 wand 来自方法后置条件。'),
'package': (
'&mut pt.x\n//@ at return: package W(q,p,y0) {\n//@   assume returned G(q,z)\n//@   execute UnblockGraph.plan(pt)\n//@   fold Point; generic Point p\n//@   check (pt.x = z /\\ pt.y = y0)\n//@ }',
'I(p,z) := cell 1 p i32_val z\nPoint(p,(u,v)) := I(field(p,x),u) **\n                  I(field(p,y),v)\nG(q,z) := GenericOwn I32 q z\nG_Point(p,s) := GenericOwn Point p s\nW(q,p,y0) := ALL z, G(q,z) -* G_Point(p,(z,y0))',
'UnblockGraph 给出恢复脚本；package 捕获未借出的 y，并验证用户 after_expiry。'),
'apply': (
'*y = 72;  //@ proof generic I32 q\n//@ assert G(q,72) ** W(q,p,y0)\n//@ proof specialize W at 72; apply_wand\n//@       concrete Point p; restore pt\n//@ assert Point(p,(72,y0))\nassert!(pt.x == pt.y); // 初始化给出 y0 = 72',
'I(p,z) := cell 1 p i32_val z\nPoint(p,(u,v)) := I(field(p,x),u) **\n                  I(field(p,y),v)\nG(q,z) := GenericOwn I32 q z\nG_Point(p,s) := GenericOwn Point p s\nW(q,p,y0) := ALL z, G(q,z) -* G_Point(p,(z,y0))',
'移除调用 coupled edge 时 apply wand，恢复 pt 和用户到期断言。'),
'history': (
'//@ p := addr_at(_before_0_8, _3)\n//@ assert G_Point(p)\n//@ proof make_concrete Point p\n//@ assert R_Point(p)\nassert!(pt.x == pt.y);',
'G_Point(p) := EX s, GenericOwn Point p s\nR_Point(p) := EX s, Own_Point(p,s)\nOwn_Point(p,(u,v)) := I(field(p,x),u) **\n                      I(field(p,y),v)\nI(p,z) := cell 1 p i32_val z\naddr_at(L,r) := (snapshot_at(L,r)).addr',
'历史标签保留被移动引用的目标；节点先按类型/region 解释，再应用 old。'),
'shared': (
'//@ assert Shared(Point,loc(pt),s)\n&pt.x\n//@ proof derive shared field view\n//@ assert Shared(I32,loc(result),r)\n//@       ** !!(r.val = s.val.x)',
'Shared(T,slot,r) := RefShared(T,slot,r) **\n                 GenericRead(T,r.addr,r.val)\nRefShared(T,slot,r) :=\n  cell 1 slot shared_addr r.addr **\n  cell 1 slot shared_meta r.meta **\n  !!(meta_type(r.meta) = Metadata(T))',
'共享借用传播读资源与快照；值等式来自用户 ensures，不产生独占目标资源。'),
'projection': (
'//@ assert Indirect(cs,rho)\n//@ PCG: expand c↓rho -> c.x↓rho\n//@ proof skip  // 本投影 expansion 无语句\n//@ assert Indirect(cs,rho)\nc.x',
'cs := snapshot(c)；rho 为 c.x 的生命周期\nIndirect(cs,rho) := EX z,\n  GenericOwn I32 cs.x.addr z\ncs.x 是引用快照；cs.x.addr 是其目标地址。\n此处只展示对应 region 的间接资源。',
'region 决定间接资源归属；当前 lifetime projection expansion 不直接 fold/unfold。'),
'flow': (
'//@ assert P\n//@ PCG: BorrowFlow / FUTURE bookkeeping\n//@ proof skip\n//@ assert P\nx',
'P : Assertion  (* 进入动作前的已有断言 *)\n{{P}} skip {{P}}\nFUTURE：供分析记录未来依赖的投影标签。\nBorrowFlow：借用/region 之间的数据流关系。\n两者在本分支不分配对象、不新增资源。',
'普通 BorrowFlow/FUTURE 参与图分析；当前分支只输出注释，Unsize 另行处理。'),
'unsize': (
'//@ assert Seq(base,[13;37;72])\nlet y: &mut [i32] = &mut x;\n//@ encode cast_array_to_slice base 3\n//@ assert Slice(base,3,[13;37;72])',
'Seq(p,vs) := SEP i, I(elem(p,i), vs[i])\nI(a,z) := cell 1 a i32_val z\nSlice(p,n,vs) := !!(length vs = n) ** Seq(p,vs)\nTy_INV_Slice(n,vs) := length vs = n /\\ n >= 0\n序列视图为规约抽象；后端表示关系仍需证明。',
'MIR 执行 Unsize；PCG 保存源、目标和转换元数据，供借用结束时 undo。'),
'index': (
'//@ assert Slice(base,3,vs) ** !!(0 <= 1 < 3)\n//@ proof open_index base 1\ny[1] = 3;\n//@ assert I(elem(base,1),3) ** Frame(base,vs,1)',
'I(a,z) := cell 1 a i32_val z\nSeq(p,vs) := SEP i, I(elem(p,i),vs[i])\nFrame(p,vs,k) := SEP i != k, I(elem(p,i),vs[i])\nSlice(p,n,vs) := !!(length vs = n) ** Seq(p,vs)\nSEP 仅遍历合法下标；frame 保留未写元素。',
'索引 expansion 暴露元素资源并保留 frame；下标与边界证明来自 MIR/类型。'),
'unsize-undo': (
'y[1] = 3;\n//@ assert Slice(base,3,[13;3;72])\n//@ proof undo_unsize base 3\n//@ assert Seq(base,[13;3;72])\ntest5_2(x);',
'Seq(p,vs) := SEP i, I(elem(p,i),vs[i])\nI(a,z) := cell 1 a i32_val z\nSlice(p,n,vs) := !!(length vs = n) ** Seq(p,vs)\nundo 保留基地址、长度和当前值。\n视图恢复不会把元素 3 改回旧值 37。',
'Unsize BorrowFlow 移除触发 undo，将切片期间修改后的内容归还数组视图。'),
'branch': (
'*r = 9;\n//@ assert I(addr(r),9)\n//@ proof if guard_a then recover a\n//@       if guard_b then recover b\n//@ assert guarded guard_a I(loc(a),9) **\n//@      guarded guard_b I(loc(b),9)',
'I(p,v) := cell 1 p i32_val v\nguarded g P := if g then P else emp\nguard_a := path_taken(bb0,bb1)\nguard_b := path_taken(bb0,bb2)\n未借出的另一变量资源保持在 frame 中。',
'PCG 边的路径条件变成守卫，只在实际执行的分支恢复相应资源。'),
'loop-invariant': (
'while i > 2 {\n    body_invariant!(i > 2);\n    //@ invariant EX n, I(loc(i),n) ** !!(n > 2)\n    //@ proof check at body cutpoint\n    i -= 1;\n    //@ proof re-establish on next body entry\n}',
'I(p,n) := cell 1 p i32_val n\nI_resource := EX n, I(loc(i),n)\nI_body := EX n, I(loc(i),n) ** !!(n > 2)\n资源项来自 used/live/initialized/capability。\nn > 2 来自用户；完整编码还含路径标志。',
'PCG 提供循环资源不变式；用户 body_invariant 提供值性质，切点位于循环体。'),
'loop-wand': (
'while again {\n    //@ Add + edge_to_loop: package W(L)\n    r = &mut *r;\n    again = false;\n}\n*r = 9;\n//@ Remove Loop: apply_wand W(L)\n//@ FAILED: wand.not.found',
'R(p) := EX v, cell 1 p i32_val v\nG(q) := EX v, GenericOwn I32 q v\nW(L) := G(addr_at(L,r)) -* R(loc(x))\nI_loop(L) := G(addr_at(L,r)) ** W(L)\nL 是循环边界标签；动作见 successor.actions。\n本例的恢复义务未通过验证。',
'LoopAbstraction 在指向循环的 Add 上 package、Remove 上 apply；本运行失败。'),
'remote': (
'while i < n {\n    //@ package (ListRes(q) -* ListRes(p_entry))\n    // current 沿 tail 前进（原程序见上方）\n}\n&mut current.head\n//@ FAILED: insufficient.permission',
'ListRes(p) := EX v, GenericOwn List p v\np_entry := addr_at(entry,self)\nq := addr_at(outer_package,current)\nW_remote := ListRes(q) -* ListRes(p_entry)\nList 是 Rust 类型；ListRes 是抽象资源。\n该运行失败，未得到已证明的恢复结论。',
'Remote 基底按入口 old 解释，参与循环归还 wand；官方链表实例验证失败。'),
'inhale-path': (
'assert!(a == 4);\n//@ panic 分支（不是正常路径）\n//@ assert P\n//@ proof check False; assume False\n//@ assert !!False',
'P : Assertion  (* panic 分支的已有状态 *)\nunreachable := !!False\n{{P}} check False {{P}}，须 P |-- !!False\n{{P}} assume False {{unreachable}}\n前一步证明不可达，后一步截断路径。',
'panic 后先检查不可达，再截断路径；这里的 inhale 不表示获取借用资源。'),
}

# Keep complete local definitions in the existing symbol dialogs.
FULL_COMPACT = COMPACT
assert set(FOCUS) == set(FULL_COMPACT)
COMPACT = {key: (ann, FOCUS[key][1], summary)
           for key, (ann, defs, summary) in FULL_COMPACT.items()}

TOKEN=re.compile(r'//[^\n]*|\(\*[\s\S]*?\*\)|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\'|\'[A-Za-z_]\w*|\b\d+\b|\b[A-Za-z_]\w*\b|\*\*|\-\*|\|--|<==>|:=|!!|->|=>')
KEY=set('fn pub let mut where struct enum match Some None while if else return use const as requires ensures assert proof at invariant Definition Parameter Rule forall exists EX ALL SEP True False emp skip check assume package apply_wand make_generic make_concrete generic concrete consume fold unfold specialize'.split())
TYPES=set('i32 u32 usize bool I32 U32 Bool Point T U List Assertion GenericOwn DirectMut DirectShared I G R Ref Seq Slice Frame Own_T R_T Own_Point R_Point G_Point W R_mut I_body I_resource I_loop FutureProjection Flow Ty_INV_I32 Ty_INV_T Ty_INV_Slice'.split())

def highlight(s,context=None):
    out=[];at=0
    for m in TOKEN.finditer(s):
        out.append(html.escape(s[at:m.start()]));v=m.group();kind=''
        if v.startswith('//@'):
            out.append('<span class="tok-marker">//@</span>'+highlight(v[3:],context));at=m.end();continue
        if v.startswith(('//','(*','/*')):kind='comment'
        elif v.startswith('"') or (v.startswith("'") and v.endswith("'")):kind='string'
        elif v.startswith("'"):kind='lifetime'
        elif v.isdigit():kind='number'
        elif v in KEY:kind='keyword'
        elif v in TYPES or v.startswith(('Ty_INV','Own_','Generic')):kind='type'
        elif re.match(r'^\s*[(!]',s[m.end():]):kind='function'
        elif not v[0].isalnum() and v[0]!='_':kind='operator'
        token='<span class="tok-'+kind+'">'+html.escape(v)+'</span>' if kind else html.escape(v)
        dest=term_target(v,context)
        if dest:token='<a class="term-link" data-term="'+dest+'" href="#'+dest+'" title="查看定义、参数与依据">'+token+'</a>'
        out.append(token);at=m.end()
    out.append(html.escape(s[at:]));return ''.join(out)

def code(s,kind,context=None):return '<pre class="'+kind+'"><code>'+highlight(s,context)+'</code></pre>'

def notation(text, previous=''):
    """Explain an operator in its first card, in section order 03 then 04."""
    symbols=[('**','分离合取'),('-*','归还 wand'),('!!','纯断言'),('|--','蕴含'),
             ('{{','前后状态契约'),('EX','存在量化'),('ALL','全称量化'),('SEP','有限分离合取')]
    labels={'{{':'{{P}} S {{Q}}'}
    return '　'.join('<span>'+html.escape(labels.get(symbol,symbol))+' '+label+'</span>'
                    for symbol,label in symbols if symbol in text and symbol not in previous)

def notation_html(text, previous=''):
    legend=notation(text,previous)
    return '<p class="notation">'+legend+'</p>' if legend else ''


def flow():
    nodes=[('stage-input',15,6,220,46,'Rust + Prusti 规约'),('stage-facts',15,72,220,46,'MIR + NLL'),('stage-pcg',15,138,220,46,'PCG 数据流'),('stage-types',15,220,105,48,'类型资源'),('stage-phases',130,220,105,48,'MIR 位置'),('stage-edges',15,310,220,46,'边 / 能力 → 操作'),('stage-calls',15,406,66,48,'调用'),('stage-casts',92,406,66,48,'转换'),('stage-control',169,406,66,48,'控制流'),('stage-viper',15,510,220,46,'Viper 验证程序'),('stage-verification',15,576,220,46,'后端验证'),('stage-export',15,674,220,46,'资源规约 / LLBC')]
    paths=['M125 52V68','M125 118V134','M125 184V204H182V216','M125 204H67V216','M67 268V288H125V306','M182 268V288H125','M125 356V380H48V402','M125 380V402','M125 380H202V402','M48 454V480H125V506','M125 454V480','M202 454V480H125','M125 556V572','M235 533H246V697H239']
    svg='<svg viewBox="0 0 250 736" aria-label="Prusti 工作流" xmlns="http://www.w3.org/2000/svg"><defs><marker id="arrow" markerWidth="6" markerHeight="6" refX="5" refY="3" orient="auto"><path d="M0 0L6 3L0 6Z" fill="#8a9db2"/></marker></defs>'
    svg+='<rect class="encoder-zone" x="6" y="199" width="238" height="292" rx="10"/>'
    svg+=''.join('<path class="connector" d="'+p+'" marker-end="url(#arrow)"/>' for p in paths)
    svg+='<text class="flow-label" x="15" y="300">Prusti encoder</text><text class="flow-label" x="15" y="397">按边种类分派</text><text class="flow-label" x="15" y="660">规约转写 · 后续导出建议</text>'
    for id,x,y,w,h,label in nodes:
        svg+=f'<g class="step" data-id="{id}" tabindex="0" role="button" aria-label="{label}" aria-pressed="false"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="7"/><text x="{x+w/2}" y="{y+h/2+6}" text-anchor="middle">{label}</text></g>'
    return svg+'</svg>'

def render(root,old_page,machine):
    esc=html.escape
    articles=[];templates=[];budgets=[];origins=[]
    for i,m in enumerate(machine,1):
        ident=m['id'];ann,defs,summary=COMPACT[ident]
        rust=textwrap.dedent('\n'.join(l for l in m['original_rust'].splitlines() if l.strip() and not l.lstrip().startswith('//')))
        full=re.search(r'<article class="case" id="'+ident+r'".*?</article>',old_page,re.S)[0]
        full=re.sub(r'<article class="case" id="[^"]+"[^>]*>','<article class="evidence-record">',full,1)
        parts=re.findall(r'<section class="part".*?</section>',full,re.S)
        assert len(parts)==5
        ordered=[]
        for number,old_index in enumerate([0,1,4,2,3],1):
            part=re.sub(r'data-part="\d"',f'data-part="{number}"',parts[old_index],count=1)
            part=re.sub(r'<h2><b>\d\d</b>',f'<h2><b>{number:02}</b>',part,count=1)
            ordered.append(part)
        full=full[:full.index(parts[0])]+''.join(ordered)+'</article>'
        # Preserve code text exactly while adding offline syntax colors.
        full=re.sub(r'<code>(.*?)</code>',lambda x:'<code>'+highlight(html.unescape(x[1]))+'</code>',full,flags=re.S)
        full=full.replace('<section class="part" data-part="4">',
                          '<section class="part" data-part="4">'+render_details(ident,ann,m,highlight),1)
        items=locate(ident,ann)
        origins.append(dict(id=ident,annotation=ann,items=[dict(item,evidence=resolve_refs(item,m)) for item in items],
                            derivation='presentation assertions summarize encoding contracts; not literal Prusti output'))
        templates.append('<template id="evidence-'+ident+'">'+full+'</template>')
        point=json.loads((root/m['point']).read_text())
        mir=point['mir']['debug_stmt']
        stage=f"{point['block']}[{point['statement_index']}] · {point['phase']}"
        phases={'pre_main':('PreMain','MIR 主效果之前'),
                'post_main':('PostMain','MIR 主效果之后'),
                'pre_operands':('PreOperands','读取 MIR 操作数之前'),
                'post_operands':('PostOperands','读取 MIR 操作数之后'),
                'initial':('Initial','该程序点的初始图状态')}
        phase,phase_reason=phases.get(point['phase'],(point['phase'],'该 MIR 程序点的分析阶段'))
        location=f"MIR：{point['block']}[{point['statement_index']}]"
        origin='官方例子' if not m['evidence_origin'].startswith('自编') else '自编实跑'
        focus_title,_,focus_reason=FOCUS[ident]
        primary,supporting=presentation_parts(ident)
        rule_html=code(primary,'definitions primary-rule',ident)+notation_html(primary)
        support_html=(code(supporting,'definitions supporting-definitions',ident)+notation_html(supporting,primary)) if supporting else '' 
        annotation_legend=notation_html(ann,defs)
        vlink=m['viper_file']+'.html#L'+str(m['viper_start_line'])
        source=f"evidence/pcg-encoding-20261125/{m['case']}/{m['case']}.rs.html#L{m['source_start_line']}"
        articles.append(f'''<article class="case" id="{ident}" data-stage="{m['stage']}">
<header class="case-head"><h1>{esc(m['rule'])}</h1></header>
<section class="card original" data-part="1"><h2><b>01</b> Rust <small>{origin} · 省略注释 / 空行</small></h2>{code(rust,'rust dense' if len(rust.splitlines()) > 10 else 'rust')}</section>
<section class="card graph" data-part="2"><h2><b>02</b> MIR / PCG <small>{esc(location)}</small></h2>{code(mir,'mir')}<p class="phase-location">PCG 阶段：<a class="term-link" data-term="term-AnalysisLocation" href="#term-AnalysisLocation">{esc(phase)}</a> · {esc(phase_reason)}</p><button class="zoom-graph" data-image="{m['graph']}" aria-label="放大 PCG 图"><img class="pcg" src="{m['graph']}" alt="{esc(stage)} 的真实 PCG 图"></button></section>
<section class="card definition" data-part="3"><h2><b>03</b> 本页关键规则 <small><a class="term-link" data-term="symbols-{ident}" href="#symbols-{ident}">完整定义 / 参数</a></small></h2><p class="focus-title">{esc(focus_title)}</p>{rule_html}<p class="focus-reason">{highlight(focus_reason,ident)}</p>{support_html}</section>
<section class="card annotation" data-part="4"><h2><b>04</b> Rust 上的资源规约 / 证明步骤 <small>来源 → 理由</small></h2>{render_annotation(ident,ann,m,lambda s:highlight(s,ident))}{annotation_legend}</section>
<section class="takeaway" data-part="5"><b>05</b><p>{esc(summary)}</p></section>
<footer class="case-footer"><nav style="margin-left:auto"><a href="{source}" target="_blank" rel="noopener">Rust</a><a href="{m['point']}" target="_blank" rel="noopener">PCG JSON</a><a href="{vlink}" target="_blank" rel="noopener">实际输出</a><button class="open-evidence" data-case="{ident}">定义 / 源码详情</button></nav></footer>
</article>''')
        budgets.append(dict(id=ident,rust_lines=len(rust.splitlines()),annotation_lines=len(ann.splitlines()),origin_notes=len(items),definition_lines=len(defs.splitlines()),summary_characters=len(summary)))
    css=(root/'assets/pcg-resource-rules.css').read_text()
    js=(root/'assets/pcg-resource-rules.js').read_text()
    options=''.join(f'<option value="{m["id"]}">{i:02} {esc(m["rule"])}</option>' for i,m in enumerate(machine,1))
    data=json.dumps([dict(id=m['id'],title=m['rule'],stage=m['stage']) for m in machine],ensure_ascii=False).replace('</','<\\/')
    page=f'''<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>PCG 资源规约 · 演示</title><style>{css}</style></head><body>
<div id="viewport"><main class="app" id="deck"><aside class="flow-panel"><a class="brand" href="index.html">CONTRACT-RUST</a><h2>Prusti 工作流</h2><div class="flow">{flow()}</div><p>Prusti 45b27b4 · PCG 495e59b</p></aside>
<div class="detail-panel"><div class="toolbar"><select id="case-select" aria-label="选择规则">{options}</select><span id="counter"></span><button id="previous" aria-label="上一页">←</button><button id="next" aria-label="下一页">→</button><button id="fullscreen">全屏</button><button id="print">打印</button></div><div id="cases">{''.join(articles)}</div></div></main></div>
<dialog id="evidence-dialog"><header><h2 id="dialog-title">定义与证据</h2><button id="close-dialog" aria-label="关闭详情">关闭 ×</button></header><div id="dialog-content"></div></dialog>
<div hidden>{''.join(templates)}{render_templates(FULL_COMPACT,code)}</div><p id="announce" aria-live="polite" class="sr-only"></p>
<script id="case-data" type="application/json">{data}</script><script>{js}</script></body></html>'''
    (root/'pcg-resource-contracts.html').write_text(page)
    audit={
      'method':'manual source/evidence review plus static consistency checks; not a Rocq proof or browser rendering',
      'fixes':[
        'Focused each definition card on one PCG-to-resource rule; retained full definitions in dialogs.',
        'Ordered cards as rule, rule notation, explanation, supporting definitions and their notation.',
        'Removed presentation status badges; preserved run outcomes in evidence and failure explanations.',
        'Introduced resource aliases visibly on first use, including I/Ref/G and changed predicate arities.',
        'Placed notation legends in their first card (03 before 04), without duplicate explanations.',
        'Expanded DirectMut inline; distinguished reference slot, target, metadata and referent resource.',
        'Separated MIR location from PCG evaluation phase; removed phase metadata from Rust assertions.',
        'Replaced implicit variable-as-address notation with loc/addr and explicit per-case parameter scopes.',
        'Replaced generic/concrete logical entailments with resource-transform contracts.',
        'Used a struct snapshot {f:=4} for T; defined Point pair notation and every G_Point/W parameter.',
        'Defined shared slot resources separately from opaque target read resources and snapshot constraints.',
        'Moved expanded-code assertions to the correct sides of their Rust effects without repeating assignments.',
        'Made Borrow Remove restoration explicit before the field assertion; distinguished MIR cast encoding from extra proof effects.',
        'Kept history and Remote in node-base interpretation rather than inventing PcgNode constructors.',
        'Documented frame bounds, conversion/storage assumptions and failed loop obligations.',
        'Added term dependency lookup, local symbol scopes and relative source/evidence links.'
      ],
      'cases':[dict(id=m['id'],context=CASE_CONTEXT[m['id']],
                    focus=dict(title=FOCUS[m['id']][0],rule=FOCUS[m['id']][1],reason=FOCUS[m['id']][2]),
                    terms=dependencies(FULL_COMPACT[m['id']][0],FULL_COMPACT[m['id']][1]),
                    source_sha256=m['source_sha256'],viper_sha256=m['viper_sha256']) for m in machine]
    }
    (root/'evidence/pcg-resource-rules/semantic-audit.json').write_text(json.dumps(audit,ensure_ascii=False,indent=2)+'\n')
    (root/'evidence/pcg-resource-rules/assertion-origins.json').write_text(json.dumps(dict(cases=origins),ensure_ascii=False,indent=2)+'\n')
    (root/'evidence/pcg-resource-rules/presentation-budget.json').write_text(json.dumps({'canvas':[1760,990],'cases':budgets,'syntax_highlighting':'embedded token spans; no CDN'},ensure_ascii=False,indent=2)+'\n')
