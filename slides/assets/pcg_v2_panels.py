"""Two-part rule cards with ordered introductions and concise reuse notes."""
import html
from pcg_assertion_origins import render_annotation
from pcg_presentation import code, highlight

# A semantic group may introduce several mutually dependent names together.
TERMS = {
 'integer': ('I / G：具体 / 抽象目标资源',[
  'I(p,v) := cell 1 p i32_val v；G(p,v) := GenericOwn I32 p v。',
  'cell sh p f v 持有字段 (p,f) 的份额 sh 与值 v；1 是完整权限。',
  'GenericOwn T p v 是类型 T 的抽象目标资源，v 是当前快照；I32 表示 i32 类型。']),
 'generic': ('generic：封装资源',[
  'make_generic / generic 按类型把具体资源封装为抽象资源，保持目标值。']),
 'reference': ('Ref / DirectMut：引用槽资源',[
  'Ref(s,p) := DirectMut I32 s (p,unit)。',
  'DirectMut T s (p,m) 持有槽 s 的地址 p 与元数据 m，不含目标资源；unit 是 i32 的空元数据。']),
 'sep': ('**：分离合取',[
  'P ** Q：同时持有可分离组合的 P、Q 资源。']),
 'annotation': ('assert / proof：检查 / 操作',[
  'assert 检查当前位置的断言；proof 标注编码器安排的资源或证明操作。']),
 'addresses': ('loc / addr：槽 / 目标地址',[
  'loc(x) 是变量 x 的槽地址；addr(x) 是引用 x 指向的目标地址，两者不同。']),
 'concrete': ('concrete：打开资源',[
  'make_concrete / concrete 按目标类型打开 GenericOwn，得到具体资源并保持值。']),
 'field': ('U / Own_T / field：字段资源与位置',[
  'U(p,v) := cell 1 p u32_val v；Own_T(q,s) := U(field(q,f),s.f)。',
  'field(q,f) 是字段 f 的逻辑地址；s.f 是结构体快照中的值。']),
 'unfold': ('unfold：展开类型资源谓词',[
  'unfold Own_T 按谓词定义暴露字段资源；PCG 投影边决定何时需要展开。']),
 'exists': ('EX：存在量化',[
  'EX s, P(s) 表示存在某个 s 满足资源断言 P(s)，不预先指定 s 的值。']),
 'hidden_struct': ('R_T：隐藏结构体快照',[
  'R_T(q) := EX s, Own_T(q,s)；持有结构体资源，但隐藏其当前字段快照。']),
 'recovery': ('ready / blocker：可回收条件',[
  'e 是待回收边；blocker 是阻塞节点。ready(e) 要求各 blocker 不再被剩余边阻塞；箭头给出归还顺序。']),
 'pure': ('!!：纯断言',[
  '!!φ 把不占用堆资源的逻辑事实 φ 放入资源断言；本例记录目标地址相等。']),
 'close': ('close / return_to：回收操作别名',[
  'close := make_generic I32；return_to := make_concrete I32；顺序由借用依赖决定。']),
 'wand': ('R / -*：目标资源与归还凭证',[
  'R(p) := EX v, GenericOwn I32 p v；隐藏当前整数值。',
  'P -* Q 是 magic wand：以后交回 P，可消耗这份凭证取得 Q；不表示立刻拥有 Q。']),
 'function_contract': ('requires / ensures：函数契约',[
  'requires 是入口所需资源；ensures 是返回后保证。\'l: \'s 表示输入生命周期覆盖返回借用。']),
 'package': ('package：证明归还',[
  'package 假设将来交回左侧资源，验证恢复右侧的步骤；return_unblock_plan 是返回处解阻计划。']),
 'old_address': ('addr_at：标签处目标',[
  'addr_at(L,r) 取标签 L 处引用 r 的目标地址；entry / return 分别是入口 / 返回状态。']),
 'point_wand': ('W / G_Point / ALL：归还契约',[
  'G_Point(p,s) := GenericOwn Point p s；(z,y0) 是 {x:=z; y:=y0} 的快照缩写。',
  'W(q,p,y0) := ALL z, G(q,z) -* G_Point(p,(z,y0))；ALL z 覆盖未来归还的任意字段值。']),
 'unblock': ('UnblockGraph.plan：按依赖恢复资源',[
  'UnblockGraph.plan(pt) 给出解阻顺序；assume returned 引入归还资源，execute 执行计划；check 检查值条件。']),
 'fold': ('fold：把字段资源组成结构体资源',[
  'fold Point 按定义重组字段资源；/\\ 表示值条件的逻辑合取。']),
 'input_point': ('P：调用前的整体资源',[
  'P(p) := EX s, G_Point(p,s)；持有整个 Point，但隐藏字段快照 s。']),
 'apply': ('|--：资源蕴含',[
  'A |-- B 表示由 A 可推出 B；本例使用 wand，将字段资源换回整体资源。']),
 'point': ('Point：展开的结构体资源',[
  'Point(p,(u,v)) := I(field(p,x),u) ** I(field(p,y),v)；u、v 分别是两字段当前值。']),
 'wand_use': ('specialize / apply_wand：实例化并使用凭证',[
  'specialize W at 72 选择归还值 z=72；apply_wand 消耗 G(q,72) 与 W，取得整体资源；restore pt 表示恢复对 pt 的访问。']),
 'invariant': ('循环资源项与用户值性质',[
  'EX n, I(loc(i),n) 保留循环变量的资源；n 是切点处当前值，不是额外的 Rust 变量。']),
 'invariant_site': ('invariant / body_invariant：循环体切点断言',[
  'body_invariant! 提供用户值性质；proof check 在循环体切点检查，再在下一次进入时重建。']),
}
RULES = {
 'borrow-create': '{{I(p,v)}} make_generic I32 p {{G(p,v)}}',
 'deref': '{{G(q,v)}} concrete q {{I(q,v)}}',
 'field-open': '{{Own_T(q,s)}} unfold Own_T\n  {{U(field(q,f),s.f)}}',
 'reborrow-chain': 'ready(e) ⇒ 回收 e，恢复被它阻塞的目标\n本例顺序：z → y → x → a',
 'basic-wand': 'requires R(p)\nensures R(q) ** (R(q) -* R(p))',
 'package': 'package (ALL z, G(q,z) -* G_Point(p,(z,y0)))',
 'call-edge': '调用前：P(p)\n调用后：R(q) ** W(q,p,y0)',
 'apply': 'G(q,72) ** W(q,p,y0)\n  |-- G_Point(p,(72,y0))',
 'loop-invariant': '资源项：EX n, I(loc(i),n)\n值性质：n > 2',
}
REASONS = {
 'borrow-create':'MIR 引用赋值安排资源转换；PCG 记录借用阻塞，常量 6 来自初始化。',
 'deref':'Deref Add 触发类型相关的资源展开；赋值 4 尚未执行。',
 'field-open':'PCG 字段投影决定展开时机，类型定义决定展开后的字段资源。',
 'reborrow-chain':'从最内层依赖开始回收，保留当前值；不要求显式 drop。',
 'basic-wand':'生命周期依赖确定归还方向：交回结果资源，才能恢复输入资源。',
 'package':'在函数返回处证明未来的归还步骤；未借出的 y 资源参与恢复。',
 'call-edge':'coupled Add 记录调用依赖；返回资源和 wand 来自方法后置条件。',
 'apply':'coupled Remove 触发 apply；归还使用当前值 72，不恢复旧字段值。',
 'loop-invariant':'分析决定应保留的资源；n > 2 来自用户规约。',
}
# Order within a slide is 03 followed by 04. Reuse within the same slide is
# explicitly distinguished from definitions on an earlier slide.
USES = {
 'borrow-create': {3:['integer','generic'],4:['reference','sep','annotation','addresses','integer','generic']},
 'deref': {3:['concrete','integer'],4:['annotation','integer','concrete','addresses']},
 'field-open': {3:['field','unfold','integer'],4:['exists','hidden_struct','field','unfold','annotation','addresses']},
 'reborrow-chain': {3:['recovery'],4:['pure','close','integer','sep','annotation','addresses','generic','concrete']},
 'basic-wand': {3:['wand','function_contract','exists','sep','integer'],4:['package','old_address','wand','function_contract','annotation']},
 'package': {3:['point_wand','point','package','wand','integer','sep'],4:['unblock','fold','point_wand','package','old_address','generic']},
 'call-edge': {3:['point_wand','input_point','wand','sep'],4:['input_point','point_wand','wand','annotation','addresses']},
 'apply': {3:['apply','point_wand','integer','sep'],4:['point','wand_use','point_wand','integer','generic','concrete','annotation','addresses']},
 'loop-invariant': {3:['invariant','exists','integer'],4:['invariant_site','invariant','pure','sep','annotation','addresses']},
}


def definitions(order):
    seen={};result={}
    for page,ident in enumerate(order,1):
        for part in (3,4):
            entries=[]
            for key in USES[ident][part]:
                fresh=key not in seen
                if fresh:seen[key]=dict(page=page,case=ident,part=part)
                entries.append(dict(key=key,new=fresh,first=seen[key],label=TERMS[key][0],paragraphs=TERMS[key][1]))
            result[ident,part]=entries
    return result


def reused_text(entries,page):
    labels=[entry['label'] for entry in entries if not entry['new']]
    return '复用：'+'；'.join(labels) if labels else ''


def glossary(entries,page,ident):
    out=[]
    for entry in entries:
        if entry['new']:
            out.append('<div class="new-definition" data-definition="'+entry['key']+'">'+''.join('<p>'+highlight(t,ident)+'</p>' for t in entry['paragraphs'])+'</div>')
    reuse=reused_text(entries,page)
    if reuse:
        out.append('<p class="reused-definitions">'+html.escape(reuse)+'</p>')
    return ''.join(out)


def render(ident,page,case,ann,params,registry):
    result=[]
    for part in (3,4):
        cls='definition' if part==3 else 'annotation'
        heading='具体规则' if part==3 else 'Rust 规约 / 证明步骤'
        upper=code(RULES[ident],'definitions primary-rule',ident)+'<p class="rule-reason">'+highlight(REASONS[ident],ident)+'</p>' if part==3 else render_annotation(ident,ann,case,lambda s:highlight(s,ident))
        lower=glossary(registry[ident,part],page,ident)
        if part==4:lower+='<div class="parameter-bindings" aria-label="本例参数">'+''.join('<p>'+highlight(line,ident)+'</p>' for line in params)+'</div>'
        result.append(f'<section class="card {cls} split-card" data-part="{part}"><h2><b>{part:02}</b> {heading}<small><a class="term-link" data-term="symbols-{ident}" href="#symbols-{ident}">完整定义</a></small></h2><div class="panel-upper">{upper}</div><div class="panel-lower"><h3>谓词、符号与参数</h3>{lower}</div></section>')
    return result


CSS = '''
.case{grid-template-rows:50px 326px minmax(0,1fr) 26px}
#package{grid-template-rows:50px 310px minmax(0,1fr) 26px}
.annotation{grid-area:3 / 1 / 4 / 8}.definition{grid-area:3 / 8 / 4 / 13}
.split-card .panel-upper{flex:none;min-width:0}.split-card .panel-lower{border-top:1px solid #c6d6df;margin-top:7px;padding-top:6px;min-width:0;flex:none}
.panel-lower h3{font-size:15px;line-height:20px;margin:0 0 3px;color:#4b687c;font-weight:650}
.split-card .definitions{font-size:19px;line-height:24px}.rule-reason{font-size:17px;line-height:22px;margin:5px 0 0;color:#42604e}
.new-definition p{font-size:17px;line-height:22px;margin:0}.reused-definitions{font-size:14px;line-height:19px;margin:3px 0 0;color:#5c7183}
.split-card .annotated{font-size:19px;line-height:22px}.split-card .annotation-line{min-height:22px}.split-card .origin-line{font-size:14px;line-height:18px}
.split-card .parameter-bindings{font-size:17px;line-height:22px;margin:4px 0 0;padding:0;border:0}
@media print{.case{grid-template-rows:44px 289px minmax(0,1fr) 24px}#package{grid-template-rows:44px 273px minmax(0,1fr) 24px}.split-card .definitions,.split-card .annotated{font-size:17px}.new-definition p,.split-card .parameter-bindings,.rule-reason{font-size:15px;line-height:20px}.reused-definitions{font-size:12px;line-height:17px}}
'''
