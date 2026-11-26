"""Render traceable PCG state comparisons from frozen Graphviz snapshots."""
from pathlib import Path
import hashlib
import html
import json
import re
import subprocess

# Each action frame is the recorded state after that numbered action.
FRAMES = {
 'borrow-create': [('pre_main','借用前','PreMain'),('post_main','借用后：建立 Borrow','PostMain')],
 'deref': [('PreMain_action_1','Deref Add 前','PreMain · action 1 后'),('PreMain_action_2','Deref Add 后','PreMain · action 2 后')],
 'field-open': [('PreMain_action_3','字段展开前：已有 *x','PreMain · action 3 后'),('PreMain_action_4','字段展开后：新增 (*x).f','PreMain · action 4 后')],
 'reborrow-chain': [('initial','回收前','Initial'),('PreOperands_action_6','归还 z，恢复 *y','PreOperands · action 6 后'),('PreOperands_action_14','归还 y，恢复 *x','PreOperands · action 14 后'),('PreOperands_action_22','归还 x，恢复 a','PreOperands · action 22 后')],
 'basic-wand': [('initial','输入到返回引用的依赖链','Initial')],
 'package': [('initial','返回点依赖图与恢复计划','Initial')],
 'call-edge': [('PostMain_action_0','coupled Add 前','PostMain · action 0 后'),('PostMain_action_1','coupled Add 后','PostMain · action 1 后')],
 'apply': [('PreOperands_action_4','coupled edge 尚在','PreOperands · action 4 后'),('PreOperands_action_5','删除 coupled edge → apply','PreOperands · action 5 后'),('PreOperands_action_10','回收其余依赖，恢复 pt','PreOperands · action 10 后')],
 'loop-invariant': [('pre_operands','不变式切点：节点能力（此快照无借用边）','PreOperands')],
}
NOTES = {
 'borrow-create':'新增橙色借用边；值 6 来自 MIR，不是图中的数值分析结果。',
 'deref':'加粗绿色边对应 Deref Add；此时尚未执行后续的 Weaken。',
 'field-open':'对比只隔一条字段 AddEdge；unfold 的内容由类型定义决定。',
 'reborrow-chain':'按 blocker 依赖回收；步骤帧取自同一程序点的真实动作记录。',
 'basic-wand':'沿入口 → 临时重借用 → RETURN 的依赖链，确定反向归还义务。',
 'package':'以下是 return_unblock_actions 的有序分组，不是执行 package 后的新程序状态。',
 'call-edge':'新增红色虚线是调用 coupled edge；wand 来自方法后置条件。',
 'apply':'删除 coupled edge 触发 apply；恢复 pt: E 还需要后续边回收。',
 'loop-invariant':'E = Exclusive：可读写且未被借用；W = Write：可覆写，不保证可读取当前值。i > 2 来自用户规约。',
}


def label_info(text):
    result={}
    for line in text.splitlines():
        m=re.match(r'^"([^"]+)" \[label=(<<FONT FACE="courier">(.*?)</FONT>>|<([^>]+)>|"([^"]*)")',line)
        if m:result[m[1]]=html.unescape(next(v for v in m.groups()[2:] if v is not None))
    return result


def graph_signature(text):
    nodes=set(label_info(text))
    edges=re.findall(r'^\s*"([^"]+)" -> "([^"]+)"',text,re.M)
    return dict(nodes=sorted(nodes),edges=sorted(edges))


def display_dot(text, ident):
    labels=label_info(text);mapping={};output=[]
    for line in text.splitlines():
        if line=='layout=dot':
            output.extend(['layout=dot','rankdir=TB' if ident=='loop-invariant' else 'rankdir=LR','ranksep=0.24','nodesep=0.18','node [fontsize=18, margin="0.07,0.04"]','edge [fontsize=13, arrowsize=0.8]']);continue
        m=re.match(r'^"([^"]+)" \[label=(<<FONT FACE="courier">(.*?)</FONT>>|<([^>]+)>|"([^"]*)")',line)
        if m:
            original=labels[m[1]]
            short=re.sub(r' before bb\d+\[\d+\](?::\w+)?','',original)
            if short!=original:short='old '+short
            mapping[m[1]]=dict(original=original,display=short)
            line=line[:m.start(2)]+json.dumps(short,ensure_ascii=False)+line[m.end(2):]
        e=re.match(r'^\s*"([^"]+)" -> "([^"]+)"',line)
        if e:
            # Retain exact endpoints, direction, colors and styles. Shorten only
            # borrow-label presentation; the original remains in its tooltip.
            bm=re.search(r'label="(bw\d+): ([^"]+)"',line)
            if bm:line=line[:bm.start()]+f'label="{bm[1]}", tooltip="{bm[1]}: {bm[2]}"'+line[bm.end():]
            focus=('color="orange"' in line or 'color="red"' in line)
            if ident=='deref':focus=e[2] in labels and labels[e[2]].startswith('*x') and 'color="green"' in line
            if ident=='field-open':focus='(*x).f' in labels.get(e[2],'') and 'color="green"' in line
            if focus:line=re.sub(r'\]\s*$',', penwidth=3.5]',line)
        output.append(line)
    return '\n'.join(output)+'\n',mapping


def build_graph_card(root, case, records):
    esc=html.escape;ident=case['id'];point=json.loads((root/case['point']).read_text())
    frozen=root/'evidence/pcg-encoding-20261125';original=frozen/point['dot']
    folder=original.parent
    block,index_at=point['block'],point['statement_index']
    if ident=='loop-invariant':
        # The encoder places the body invariant at bb6, not the original head bb1.
        block,index_at='bb6',0
    location=f'{block}[{index_at}]'
    stem=f'{block}_stmt_{index_at}_' 
    out=root/'evidence/pcg-resource-v2/graphs';out.mkdir(parents=True,exist_ok=True)
    frames=[]
    for index,(suffix,title,phase) in enumerate(FRAMES[ident]):
        source=folder/(stem+suffix+'.dot')
        if not source.exists() and ident=='basic-wand':source=root/'evidence/pcg-resource-rules/basic-return.dot'
        raw=source.read_text();modified,mapping=display_dot(raw,ident)
        # Label rewriting must never introduce or remove nodes or edges.
        assert graph_signature(raw)==graph_signature(modified)
        name=f'{ident}-{index}';dot=out/(name+'.dot');svg=out/(name+'.svg');original_svg=out/(name+'-original.svg')
        dot.write_text(modified)
        subprocess.run(['dot','-Tsvg',str(dot),'-o',str(svg)],check=True)
        subprocess.run(['dot','-Tsvg',str(source),'-o',str(original_svg)],check=True)
        rel=lambda p:p.relative_to(root).as_posix()
        action=None
        if '_action_' in suffix:
            ai=int(suffix.rsplit('_',1)[1]);phase_key=re.sub(r'(?<!^)(?=[A-Z])','_',suffix.split('_action_')[0]).lower()
            if phase_key==point['phase']:actions=point['phase_actions']
            else:
                data=json.loads((folder/'pcg_data.json').read_text())
                actions=data[point['block']]['statements'][point['statement_index']]['actions'][phase_key]
            action=actions[ai]['action']['data']['kind']
        records.append(dict(case=ident,index=index,title=title,location=location,phase=phase,source_dot=rel(source),source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),display_dot=rel(dot),display_svg=rel(svg),original_svg=rel(original_svg),labels=mapping,signature=graph_signature(raw),action=action))
        frames.append(f'<figure class="pcg-frame" data-frame="{index}"><figcaption><strong>{esc(title)}</strong><span>{esc(phase)}</span></figcaption><button class="zoom-graph" data-image="{rel(original_svg)}" aria-label="查看完整原图：{esc(title)}"><img class="pcg" src="{rel(svg)}" alt="{esc(title)}：{esc(phase)}"></button><a class="dot-source" href="{rel(source)}" target="_blank" rel="noopener">原始 DOT</a></figure>')
    mode='pair' if len(frames)==2 else 'steps' if len(frames)>2 else 'single'
    controls=''
    if mode=='steps':
        controls='<div class="graph-steps" role="group" aria-label="选择回收步骤">'+''.join(f'<button type="button" data-graph-step="{i}" aria-pressed="{str(i==0).lower()}">{i+1} {esc(spec[1])}</button>' for i,spec in enumerate(FRAMES[ident]))+'</div>'
    supplement=''
    if ident=='package':
        plan=point['return_unblock_actions'];assert len(plan)==8 and '(*pt).x' in plan[4] and '(*pt).y' in plan[5]
        supplement='<div class="unblock-plan"><span><b>1–5</b> 回收 RETURN 到字段 x 的重借用链</span><span><b>6</b> 收拢 x、y 投影</span><span><b>7–8</b> 收回解引用并恢复入口依赖</span></div>'
    if ident=='loop-invariant':
        live=point['loop_data']['live_loop_places']['usages']
        data=json.loads((folder/'pcg_data.json').read_text())
        cap=data[block]['loop_invariant_capabilities']['bb1']
        assert live['i']=='Exclusive' and cap['i']=='Exclusive'
        assert all(value=='Write' for name,value in cap.items() if name!='i')
        assert graph_signature(raw)['edges']==[]
        ignored=', '.join(sorted(name for name in cap if name!='i'))
        supplement='<table class="loop-facts"><tr><th>来源</th><th>本例结果</th><th>编码作用</th></tr><tr><td>循环活性（bb1）</td><td>i: Exclusive</td><td>选择跨迭代保留的资源</td></tr><tr><td>切点能力（bb6）</td><td>i: E；'+esc(ignored)+': W</td><td>为 i 编码类型资源；跳过 W 项</td></tr><tr><td>用户 body_invariant</td><td>i &gt; 2</td><td>在 bb6 组合资源项与值性质</td></tr></table>'
    mir=point['mir']['debug_stmt']
    if ident=='loop-invariant':
        mir='body_invariant 切点 bb6[0]；生成 invariant 于 Viper label bb_6'
    return f'<section class="card graph graph-{ident}" data-part="2"><h2><b>02</b> PCG 图与动作 <small>MIR：{esc(location)}</small></h2><p class="graph-mir">{esc(mir)}</p>{controls}<div class="graph-frames {mode}">'+''.join(frames)+f'</div>{supplement}<p class="graph-note">{esc(NOTES[ident])}</p></section>'


CSS = '''
.case{grid-template-columns:repeat(12,minmax(0,1fr));grid-template-areas:none}
.case-head{grid-area:1 / 1 / 2 / 13}.original{grid-area:2 / 1 / 3 / 5}.graph{grid-area:2 / 5 / 3 / 13}.annotation{grid-area:3 / 1 / 4 / 7}.definition{grid-area:3 / 7 / 4 / 13}.case-footer{grid-area:4 / 1 / 5 / 13}
.original pre{font-size:20px;line-height:1.25}.original pre.dense{font-size:18px}
.original h2 small{font-size:12px}.graph-mir{font:14px/18px monospace;margin:0 0 5px;overflow-wrap:anywhere}
.graph-frames{display:flex;gap:12px;flex:1;min-height:0}.pcg-frame{display:flex;flex-direction:column;min-width:0;min-height:0;flex:1;margin:0;border:1px solid #e1e7ee;border-radius:5px;padding:4px 6px}
.pcg-frame figcaption{display:flex;justify-content:space-between;gap:6px;flex-wrap:wrap;font-size:15px;line-height:19px;flex:none}.pcg-frame figcaption span{font-size:12px;color:#65798f}.pcg-frame .zoom-graph{margin-top:3px}.dot-source{font-size:11px;line-height:14px;align-self:flex-end}
.graph-steps{display:flex;gap:5px;margin:0 0 5px;flex:none}.graph-steps button{font-size:13px;padding:3px 7px}.graph-steps button[aria-pressed=true]{background:#dcebf9;border-color:#487fab}
.graph-frames.steps .pcg-frame:not([data-frame="0"]){display:none}.graph-frames.steps .pcg-frame.is-selected{display:flex}.graph-frames.steps .pcg-frame.is-hidden{display:none}
.graph-note{font-size:15px;line-height:21px;margin:5px 0 0;flex:none}.graph-key{font-size:12px;line-height:17px;color:#687d91;margin:2px 0 0;flex:none}
.unblock-plan{display:flex;gap:8px;font-size:14px;line-height:20px;margin-top:5px}.unblock-plan span{flex:1;background:#f0f5fa;padding:4px 6px}.unblock-plan b{color:#245f8b}
.loop-facts{border-collapse:collapse;font-size:14px;line-height:20px;flex:none;margin-top:6px}.loop-facts td,.loop-facts th{text-align:left;padding:3px 7px;border-bottom:1px solid #dce4ec}.loop-facts th{color:#365b78}
@media print{.original pre{font-size:17px}.original pre.dense{font-size:16px}.graph-note{font-size:13px}.graph-key{font-size:10px}.graph-steps button{font-size:11px}.graph-frames.steps{gap:4px}.pcg-frame figcaption{font-size:11px;line-height:15px}.pcg-frame figcaption span{font-size:10px}.graph-frames.steps .dot-source{display:none}}
'''
JS = '''
// Frame selection stays local to its slide and does not change slide navigation.
document.querySelectorAll('[data-graph-step]').forEach(button=>button.addEventListener('click',()=>{
 const card=button.closest('.graph'),index=button.dataset.graphStep;
 card.querySelectorAll('[data-graph-step]').forEach(b=>b.setAttribute('aria-pressed',String(b===button)));
 card.querySelectorAll('[data-frame]').forEach(frame=>{
  const chosen=frame.dataset.frame===index;
  frame.classList.toggle('is-selected',chosen);frame.classList.toggle('is-hidden',!chosen);
 });
}));
'''
