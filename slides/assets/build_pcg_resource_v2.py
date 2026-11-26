#!/usr/bin/env python3
"""Build the concise presentation from frozen cases and the maintained full deck."""
from pathlib import Path
import html
import json
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / 'assets'))
from pcg_presentation import COMPACT, FULL_COMPACT, code, highlight, notation_html
from pcg_assertion_origins import render_details
import pcg_terms
import pcg_v2_graphs
import pcg_v2_panels

# Order follows resource creation, access, return, then modular verification.
CASES = {
 'borrow-create': ('建立借用：引用槽与目标资源',
  'I(p,v) := cell 1 p i32_val v\nG(p,v) := GenericOwn I32 p v\nRef(s,p) := DirectMut I32 s (p,unit)',
  'I 是具体整数资源，G 是抽象目标资源；Ref 只拥有引用槽 s，记录目标 p。PCG 记录借用依赖，值 6 来自 MIR 初始化。'),
 'deref': ('解引用：按类型打开资源',
  '{{G(q,v)}} concrete q {{I(q,v)}}',
  'q=addr(x)。Deref Add 表示需要访问目标；Prusti 按 i32 类型选择 make_concrete。转换保留当前值 v，写入 4 随后才发生。'),
 'field-open': ('字段访问：展开结构体谓词',
  '{{Own_T(q,s)}} unfold {{U(field(q,f),s.f)}}',
  'PCG 字段投影决定展开时机，类型定义决定暴露什么资源。field(q,f) 是字段位置；s.f 是快照中的字段值。'),
 'reborrow-chain': ('借用结束：按依赖归还',
  '回收顺序：z → y → x → a\nready(e) := blocker 均不被剩余边阻塞',
  '先收回最内层借用，再恢复外层访问。close / return_to 对应 generic / concrete；回收由依赖与活性决定，无需显式 drop。'),
 'basic-wand': ('跨函数借用：返回资源与归还凭证',
  'R(p) := EX v, GenericOwn I32 p v\n返回：R(q) ** (R(q) -* R(p))',
  'R 隐藏目标的当前整数值；p 是输入目标，q 是返回目标。生命周期依赖决定归还方向：交回 R(q)，就可使用 wand 恢复 R(p)。'),
 'call-edge': ('函数调用：接收返回资源与 wand',
  '调用前：P(p)\n调用后：R(q) ** W(q,p,y0)',
  'P(p) 是隐藏字段快照的 Point 资源。coupled edge 记录输入与输出的依赖；调用后置条件提供 R 与 W，Add 分支不另做 package。'),
 'apply': ('apply：用归还凭证恢复原对象',
  'G(q,72) ** W(q,p,y0)\n  |-- G_Point(p,(72,y0))',
  'Point(p,(u,v)) 是展开后的两字段资源。coupled Remove 触发 call_wands / apply，交回字段资源后恢复 Point；72 来自 MIR 写入。'),

}
SUPPORT = {
 'field-open': 'U(p,v) := cell 1 p u32_val v // u32 字段资源\nOwn_T(q,s) := U(field(q,f),s.f) // 结构体资源\nR_T(q) := EX s, Own_T(q,s) // 隐藏结构体快照',
 'package': 'G_Point(p,s) := GenericOwn Point p s',
 'call-edge': 'P(p) := EX s, G_Point(p,s)',
 'apply': 'Point(p,(u,v)) := I(field(p,x),u) **\n                  I(field(p,y),v)',
}
# Introduce primitive predicates visibly once; later slides reuse their names.
PREDICATE_NOTES = {
 'borrow-create': [
  'cell sh p f v：字段 (p,f) 的权限份额 sh 与值 v；1 表示完整权限。',
  'GenericOwn T p v：类型 T 的目标 p 的抽象资源，快照为 v。',
  'DirectMut T s (p,m)：引用槽 s 的资源，记录地址 p 与元数据 m；不含目标资源。',
 ],
}
# Bind logical names to the actual Rust variables at the state in this case.
PARAMETERS = {
 'borrow-create': ['p=loc(a)，s=loc(x)，addr(x)=p，v=6。'],
 'deref': ['q=addr(x)=loc(a)；v 是写入 4 之前的目标值。'],
 'field-open': ['x 借用 a：q=addr(x)=loc(a)，s={f:=v}，field(q,f) 对应 a.f。'],
 'reborrow-chain': ['q=addr(z)=addr(y)=addr(x)=loc(a)；x、y、z 是引用变量。'],
 'basic-wand': ['p=addr_at(entry,num)，q=addr_at(return,result)=p。',
                'result 表示返回引用；v 是目标的当前整数值。'],
 'package': ['pt 是输入引用；p=addr_at(entry,pt)，q=addr_at(return,result)=field(p,x)。',
             'y0=入口 pt.y 的值；z=将来归还时 pt.x 的值。'],
 'call-edge': ['p=loc(pt)，q=addr(y)=field(p,x)；y 是指向 pt.x 的返回引用。',
               'y0=调用前 pt.y 的值，与变量 y 无关；loc(y) 是引用槽。'],
 'apply': ['p=loc(pt)，q=addr(y)=field(p,x)。',
           'y0=调用前 pt.y=72；当前 pt.x=72（由 *y 写入）。'],
 'loop-invariant': ['i 是 Rust 循环变量；n 是循环体切点处 i 的当前值。'],
}

STAGES = {'borrow-create':'stage-pcg','deref':'stage-edges','field-open':'stage-edges',
          'reborrow-chain':'stage-pcg','basic-wand':'stage-edges','package':'stage-viper',
          'call-edge':'stage-edges','apply':'stage-viper','loop-invariant':'stage-facts'}


def workflow():
    nodes=[('stage-input',16,'Rust + Prusti 规约'),('stage-facts',120,'MIR + 借用检查'),
           ('stage-pcg',224,'PCG 借用分析'),('stage-edges',368,'Prusti 编码器'),
           ('stage-viper',512,'Viper 验证程序'),('stage-verification',632,'后端验证')]
    out=['<svg viewBox="0 0 250 736" aria-label="Prusti 核心工作流" xmlns="http://www.w3.org/2000/svg"><defs><marker id="arrow" markerWidth="6" markerHeight="6" refX="5" refY="3" orient="auto"><path d="M0 0L6 3L0 6Z" fill="#8a9db2"/></marker></defs>']
    for (_,y,_),(_,ny,_) in zip(nodes,nodes[1:]):
        out.append(f'<path class="connector" d="M125 {y+48}V{ny-5}" marker-end="url(#arrow)"/>')
    for ident,y,label in nodes:
        out.append(f'<g class="step" data-id="{ident}" tabindex="0" role="button" aria-label="{label}" aria-pressed="false"><rect x="15" y="{y}" width="220" height="48" rx="7"/><text x="125" y="{y+30}" text-anchor="middle">{label}</text></g>')
    for y,label in [(301,'图状态 · 依赖 · 访问能力'),(443,'结合类型 / region'),(465,'按 MIR 位置 × 分析阶段插入'),(588,'资源谓词 · fold / unfold · wand')]:
        out.append(f'<rect x="16" y="{y-15}" width="218" height="21" fill="#f4f7fb"/>')
        out.append(f'<text class="flow-label" x="125" y="{y}" text-anchor="middle">{label}</text>')
    return ''.join(out)+'</svg>'


def main():
    old=(ROOT/'pcg-resource-contracts.html').read_text()
    cases={c['id']:c for c in json.loads((ROOT/'evidence/pcg-resource-rules/derivations.json').read_text())['cases']}
    articles=[];templates=[];evidence=[];graph_records=[]
    registry=pcg_v2_panels.definitions(CASES)
    for page_number,(ident,(title,rule,reason)) in enumerate(CASES.items(),1):
        m=cases[ident]
        card=re.search(r'<article class="case" id="'+ident+r'".*?</article>',old,re.S)[0]
        card=re.sub(r'data-stage="[^"]+"',f'data-stage="{STAGES[ident]}"',card,count=1)
        card=re.sub(r'<h1>.*?</h1>',f'<h1>{html.escape(title)}</h1>',card,count=1)
        card=re.sub(r'<section class="takeaway".*?</section>','',card,flags=re.S)
        graph_card=pcg_v2_graphs.build_graph_card(ROOT,m,graph_records)
        card=re.sub(r'<section class="card graph".*?</section>',lambda _:graph_card,card,flags=re.S)
        ann=COMPACT[ident][0]
        definition,annotation=pcg_v2_panels.render(ident,page_number,m,ann,PARAMETERS[ident],registry)
        card=re.sub(r'<section class="card definition".*?</section>',lambda _:definition,card,flags=re.S)
        card=re.sub(r'<section class="card annotation".*?</section>',lambda _:annotation,card,flags=re.S)
        articles.append(card)
        source=f'evidence/pcg-encoding-20261125/{m["case"]}/{m["case"]}.rs.html#L{m["source_start_line"]}'
        links=[('Rust',source),('PCG JSON',m['point']),('Viper',m['viper_file']+'.html#L'+str(m['viper_start_line']))]
        detail='<section class="sources"><h2>'+html.escape(title)+'</h2><nav>'+''.join('<a href="'+url+'" target="_blank" rel="noopener">'+label+'</a>' for label,url in links)+'</nav></section>'
        detail+=render_details(ident,ann,m,lambda s:highlight(s,ident))
        templates.append('<template id="evidence-'+ident+'">'+detail+'</template>')
        evidence.append({k:m[k] for k in ['id','case','point','graph','source_sha256','viper_file','viper_sha256','run_exit_code']})
        evidence[-1]['parameter_bindings']=PARAMETERS[ident]
    # Keep the existing offline navigation and exact layout, with fewer cases.
    prefix=old[:old.index('<div id="viewport">')]
    prefix=re.sub(r'<title>.*?</title>','<title>Prusti / PCG 核心规则 · v2</title>',prefix)
    prefix=prefix.replace('</style>','.predicate-notes{font-size:18px;line-height:26px;color:#40594b;margin-top:8px}.predicate-notes p{margin:0}@media print{.predicate-notes{font-size:16px;line-height:23px}}</style>')
    prefix=prefix.replace('</style>','''
.case{grid-template-rows:50px 366px minmax(0,1fr) 26px;grid-template-areas:"head head" "rust graph" "annotation definition" "footer footer"}
.parameter-bindings{font-size:18px;line-height:26px;color:#315876;border-bottom:1px solid #c8d8e8;padding-bottom:7px;margin-bottom:8px;flex:none}
.parameter-bindings p{margin:0}
@media print{.case{grid-template-rows:44px 325px minmax(0,1fr) 24px}.parameter-bindings{font-size:16px;line-height:23px}}
</style>''')
    prefix=prefix.replace('</style>',pcg_v2_graphs.CSS+pcg_v2_panels.CSS+'</style>')
    options=''.join(f'<option value="{ident}">{i:02} {html.escape(spec[0])}</option>' for i,(ident,spec) in enumerate(CASES.items(),1))
    toolbar='<div class="toolbar"><select id="case-select" aria-label="选择规则">'+options+'</select><span id="counter"></span><button id="previous" aria-label="上一页">←</button><button id="next" aria-label="下一页">→</button><button id="fullscreen">全屏</button><button id="print">打印</button></div>'
    # These descriptions address only the current encoder; the full deck retains
    # its separate discussions of other backends.
    pcg_terms.TERMS['cell']['meaning']='sh 是权限份额，1 表示完整写权限；p 是逻辑对象地址，f 是字段标识，v 是当前值。cell 描述该逻辑字段的权限与值。'
    for item in pcg_terms.TERMS.values():
        item['meaning']=re.sub(r'[^。]*VST[^。]*。','',item['meaning'])
    terms=pcg_terms.render_templates({key:FULL_COMPACT[key] for key in CASES},code)
    data=[dict(id=key,title=spec[0],stage=STAGES[key]) for key,spec in CASES.items()]
    js=(ROOT/'assets/pcg-resource-rules.js').read_text()
    defaults={'stage-input':'borrow-create','stage-facts':'borrow-create','stage-pcg':'reborrow-chain','stage-edges':'deref','stage-viper':'apply','stage-verification':'apply'}
    js=re.sub(r'const defaults=\{[^\n]*\};','const defaults='+json.dumps(defaults)+';',js)
    js=js.replace("' · PCG 资源规约'","' · PCG 核心规则 v2'")
    js+='\n'+pcg_v2_graphs.JS
    page=prefix+'<div id="viewport"><main class="app" id="deck"><aside class="flow-panel"><a class="brand" href="index.html">PRUSTI / PCG · V2</a><h2>从借用到证明</h2><div class="flow">'+workflow()+'</div><p>Prusti 45b27b4 · PCG 495e59b</p></aside><div class="detail-panel">'+toolbar+'<div id="cases">'+''.join(articles)+'</div></div></main></div>'
    page+='<dialog id="evidence-dialog"><header><h2 id="dialog-title">定义与证据</h2><button id="close-dialog">关闭 ×</button></header><div id="dialog-content"></div></dialog><div hidden>'+''.join(templates)+terms+'</div><p id="announce" aria-live="polite" class="sr-only"></p>'
    page+='<script id="case-data" type="application/json">'+json.dumps(data,ensure_ascii=False)+'</script><script>'+js+'</script></body></html>'
    assert not re.search(r'LLBC|VST',page,re.I)
    out=ROOT/'pcg-resource-contracts-v2.html';out.write_text(page)
    evidence_dir=ROOT/'evidence/pcg-resource-v2';evidence_dir.mkdir(exist_ok=True)
    (evidence_dir/'cases.json').write_text(json.dumps({'cases':evidence,'annotation_semantics':'curated resource notation with links to frozen Prusti output'},ensure_ascii=False,indent=2)+'\n')
    (evidence_dir/'graph-frames.json').write_text(json.dumps({'frames':graph_records,'display_policy':'Exact topology and colors; shortened labels, emphasized edges and horizontal layout. Original snapshots available on click.'},ensure_ascii=False,indent=2)+'\n')
    (evidence_dir/'symbol-introductions.json').write_text(json.dumps({'panels':[dict(case=ident,part=part,entries=entries) for (ident,part),entries in registry.items()]},ensure_ascii=False,indent=2)+'\n')
    print('Built',len(CASES),'core slides:',out.name)

if __name__=='__main__':main()
