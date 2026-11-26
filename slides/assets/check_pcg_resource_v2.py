#!/usr/bin/env python3
"""Validate the concise deck, its local evidence, and conservative text budgets."""
from pathlib import Path
from html.parser import HTMLParser
from urllib.parse import urlsplit, unquote
import hashlib
import html
import json
import re
import textwrap
from PIL import ImageFont
from build_pcg_resource_v2 import ROOT, CASES, PARAMETERS
from pcg_presentation import COMPACT
from pcg_v2_panels import RULES, REASONS, definitions, reused_text
from pcg_assertion_origins import locate
from pcg_v2_graphs import FRAMES, graph_signature

class Document(HTMLParser):
    def __init__(self):
        super().__init__();self.ids=[];self.urls=[]
    def handle_starttag(self,tag,attrs):
        attrs=dict(attrs)
        if 'id' in attrs:self.ids.append(attrs['id'])
        for key in ['href','src','data-image']:
            if key in attrs:self.urls.append(attrs[key])


def main():
    page=ROOT/'pcg-resource-contracts-v2.html';text=page.read_text();doc=Document();doc.feed(text)
    errors=[]
    if len(doc.ids)!=len(set(doc.ids)):errors.append('Duplicate IDs')
    if re.search(r'LLBC|VST',text,re.I):errors.append('Out-of-scope content')
    if re.search(r'class="status ',text):errors.append('Presentation status badges')
    if '沿用原图节点、边与颜色' in text:errors.append('Removed graph note remains')
    if 'data-part="5"' in text or '<section class="takeaway"' in text:errors.append('Summary section remains')
    for url in doc.urls:
        u=urlsplit(url)
        if u.scheme or u.netloc or u.path.startswith('/'):
            errors.append('Nonportable URL: '+url);continue
        target=(ROOT/unquote(u.path)).resolve() if u.path else page
        if not target.is_relative_to(ROOT.resolve()) or not target.exists():errors.append('Missing local target: '+url)
        if not u.path and u.fragment and unquote(u.fragment) not in doc.ids:errors.append('Missing fragment: '+url)
    machine={m['id']:m for m in json.loads((ROOT/'evidence/pcg-resource-rules/derivations.json').read_text())['cases']}
    cards=re.findall(r'<article class="case" id="([^"]+)".*?</article>',text,re.S)
    if cards!=list(CASES):errors.append('Case order')
    frame_report=json.loads((ROOT/'evidence/pcg-resource-v2/graph-frames.json').read_text())['frames']
    for frame in frame_report:
        raw=(ROOT/frame['source_dot']).read_bytes()
        if hashlib.sha256(raw).hexdigest()!=frame['source_sha256']:errors.append('Graph source hash: '+frame['case'])
        if graph_signature(raw.decode())!=graph_signature((ROOT/frame['display_dot']).read_text()):
            errors.append('Changed graph topology: '+frame['case'])
        if not (ROOT/frame['display_svg']).is_file() or not (ROOT/frame['original_svg']).is_file():
            errors.append('Missing rendered graph: '+frame['case'])
    fonts={}
    def lines(value,size,width=676):
        count=0
        for line in value.splitlines():
            x=0;count+=1
            for char in line:
                key=size,ord(char)>255
                if key not in fonts:fonts[key]=ImageFont.truetype('/mnt/c/Windows/Fonts/msyh.ttc' if key[1] else '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf',size)
                w=fonts[key].getlength(char)
                if x+w>width:count+=1;x=0
                x+=w
        return count
    registry=definitions(CASES)
    layouts=[];origins=0
    for page_number,ident in enumerate(CASES,1):
        rule=RULES[ident];reason=REASONS[ident]
        m=machine[ident];ann=COMPACT[ident][0];notes=locate(ident,ann);origins+=len(notes)
        source=ROOT/f'evidence/pcg-encoding-20261125/{m["case"]}/{m["case"]}.rs'
        for path,key in [(source,'source_sha256'),(ROOT/m['viper_file'],'viper_sha256')]:
            if hashlib.sha256(path.read_bytes()).hexdigest()!=m[key]:errors.append('Evidence hash: '+ident)
        card=re.search(r'<article class="case" id="'+ident+r'".*?</article>',text,re.S)[0]
        if card.count('class="origin-line"')!=len(notes):errors.append('Missing assertion provenance: '+ident)
        if re.findall(r'data-part="(\d)"',card)!=list('1234'):errors.append('Section order: '+ident)
        if 'tok-' not in card:errors.append('Missing syntax highlighting: '+ident)
        if card.count('class="pcg-frame"')!=len(FRAMES[ident]):errors.append('Missing comparison frames: '+ident)
        steps=re.findall(r'<button[^>]*data-graph-step="(\d+)"',card)
        if len(FRAMES[ident])>2 and steps!=[str(i) for i in range(len(FRAMES[ident]))]:errors.append('Step controls: '+ident)
        if ident=='package' and 'unblock-plan' not in card:errors.append('Missing return plan')
        if ident=='loop-invariant' and 'loop-facts' not in card:errors.append('Missing loop analysis')
        bindings=re.search(r'<div class="parameter-bindings"[^>]*>(.*?)</div>',card,re.S)
        if not bindings:errors.append('Missing parameter bindings: '+ident)
        elif any(line not in html.unescape(re.sub('<[^>]*>','',bindings[1])) for line in PARAMETERS[ident]):
            errors.append('Incomplete parameter bindings: '+ident)
        for part,kind in [(3,'definition'),(4,'annotation')]:
            panel=re.search(r'<section class="card '+kind+r' split-card".*?</section>',card,re.S)
            if not panel:
                errors.append('Missing split panel: '+ident+str(part));continue
            upper,lower=panel[0].split('<div class="panel-lower">',1)
            if 'class="panel-upper"' not in upper:errors.append('Missing rule area: '+ident+str(part))
            if 'new-definition' in upper or 'parameter-bindings' in upper:errors.append('Definitions in rule area: '+ident+str(part))
            plain=html.unescape(re.sub('<[^>]*>','',lower))
            if re.search(r'（(?:第 \d+ 页|本页) 0[34]）',plain):errors.append('Page reference remains: '+ident)
            if '{{P}} S {{Q}}' in plain:errors.append('Contract notation explanation remains: '+ident)
            for entry in registry[ident,part]:
                first=entry['first']
                if entry['new']:
                    if any(p not in plain for p in entry['paragraphs']):errors.append('Incomplete first definition: '+ident+':'+entry['key'])
                else:
                    if entry['label'] not in plain:
                        errors.append('Missing reuse explanation: '+ident+':'+entry['key'])
                    if (first['page'],first['part'])>=(page_number,part):errors.append('Forward definition reference: '+ident+':'+entry['key'])
        rust=textwrap.dedent('\n'.join(line for line in m['original_rust'].splitlines() if line.strip() and not line.lstrip().startswith('//')))
        size=18 if len(rust.splitlines())>10 else 20
        rh=lines(rust,size,434)*size*1.25
        def glossary_height(part):
            entries=registry[ident,part]
            width=555 if part==3 else 797
            height=37+sum(lines(p,17,width)*22 for e in entries if e['new'] for p in e['paragraphs'])
            reuse=reused_text(entries,page_number)
            if reuse:height+=3+lines(reuse,14,width)*19
            return height
        ah=lines(ann,19,797)*22+sum(lines('// '+n['source']+' → '+n['reason'],14,797)*18 for n in notes)
        bh=4+sum(lines(line,17,797)*22 for line in PARAMETERS[ident])
        ah+=glossary_height(4)+bh
        dh=lines(rule,19,555)*24+5+lines(reason,17,555)*22+glossary_height(3)
        budget=406 if ident=='package' else 390
        fit=rh<=(246 if ident=='package' else 262) and ah<=budget and dh<=budget
        if not fit:errors.append('Text budget: '+ident)
        layouts.append(dict(id=ident,rust_height=rh,annotation_height=ah,parameter_bindings_height=bh,definition_height=dh,fits=fit))
    report=dict(errors=errors,cases=len(CASES),local_urls_checked=len(doc.urls),evidence_hash_pairs_checked=len(CASES),graph_frames_checked=len(frame_report),assertion_origins=origins,deck_sha256=hashlib.sha256(page.read_bytes()).hexdigest(),text_measurement='Conservative character wrapping with installed fonts; not browser rendering',layouts=layouts,browser_rendering='Unavailable: installed Chromium lacks NSS/NSPR libraries')
    (ROOT/'evidence/pcg-resource-v2/validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k!='layouts'},ensure_ascii=False))
    if errors:raise SystemExit(1)

if __name__=='__main__':main()
