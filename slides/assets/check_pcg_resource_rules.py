#!/usr/bin/env python3
"""Check static links, extracted snippets and annotation provenance."""
from pathlib import Path
from html.parser import HTMLParser
from urllib.parse import urlsplit, unquote
import hashlib, json, re

ROOT=Path(__file__).resolve().parent.parent
OUT=ROOT/'evidence/pcg-resource-rules'
class Document(HTMLParser):
    def __init__(self):
        super().__init__();self.ids=[];self.links=[];self.codes=[];self.inside=False
    def handle_starttag(self,tag,attrs):
        a=dict(attrs)
        if 'id' in a:self.ids.append(a['id'])
        for k in ['href','src']:
            if a.get(k):self.links.append(a[k])
        if tag=='code':self.inside=True;self.codes.append('')
    def handle_endtag(self,tag):
        if tag=='code':self.inside=False
    def handle_data(self,s):
        if self.inside:self.codes[-1]+=s
cache={}
def parse(p):
    if p not in cache:
        d=Document();d.feed(p.read_text());cache[p]=d
    return cache[p]

def main():
    deck=ROOT/'pcg-resource-contracts.html'
    doc=parse(deck);errors=[];count=0
    if len(doc.ids)!=len(set(doc.ids)):errors.append('Duplicate HTML ids')
    # validation.json is the report being written below.
    for url in doc.links:
        u=urlsplit(url)
        if u.scheme or u.netloc:
            if u.scheme not in ['http','https']:errors.append('Nonportable URL '+url)
            continue
        if u.path.startswith('/'):errors.append('Absolute local URL '+url)
        p=(ROOT/unquote(u.path)).resolve() if u.path else deck
        if not p.is_relative_to(ROOT.resolve()):errors.append('Escaping URL '+url)
        elif not p.exists() and p!=OUT/'validation.json':errors.append('Missing file '+url)
        elif u.fragment and p.suffix=='.html' and unquote(u.fragment) not in parse(p).ids:errors.append('Missing anchor '+url)
        count+=1
    deriv=json.loads((OUT/'derivations.json').read_text())
    for c in deriv['cases']:
        src=ROOT/'evidence/pcg-encoding-20261125'/c['case']/(c['case']+'.rs')
        if hashlib.sha256(src.read_bytes()).hexdigest()!=c['source_sha256']:errors.append('Source hash '+c['id'])
        if c['original_rust'] not in src.read_text():errors.append('Rust excerpt '+c['id'])
        original='\n'.join(l for l in c['annotated_rust'].splitlines() if not l.lstrip().startswith('//@'))
        if original!=c['original_rust']:errors.append('Annotation changed Rust '+c['id'])
        if c['annotated_rust'] not in doc.codes:errors.append('Missing annotation '+c['id'])
        if hashlib.sha256((ROOT/c['viper_file']).read_bytes()).hexdigest()!=c['viper_sha256']:errors.append('Viper hash '+c['id'])
        if c['run_exit_code']!=json.loads((src.parent/'run.json').read_text())['exit_code']:errors.append('Run status '+c['id'])
    required={'deref','borrow-end','reborrow-chain','basic-wand','call-edge','apply','loop-wand','history','remote'}
    if not required.issubset({c['id'] for c in deriv['cases']}):errors.append('Missing required rule case')
    presentation=json.loads((OUT/'presentation-budget.json').read_text())
    main_cards=re.findall(r'<article class="case"[^>]*>(.*?)</article>',deck.read_text(),re.S)
    highlighted=sum('tok-keyword' in card and 'tok-number' in card for card in main_cards)
    if len(main_cards)!=len(deriv['cases']):errors.append('Presentation case count')
    if any('tok-' not in card for card in main_cards):errors.append('Missing syntax highlighting')
    from pcg_presentation import COMPACT, FULL_COMPACT, notation
    from pcg_rule_focus import FOCUS, presentation_parts
    from pcg_assertion_origins import locate, resolve_refs
    from pcg_terms import TERMS, CASE_CONTEXT, dependencies
    origin_count=0
    for card,c in zip(main_cards,deriv['cases']):
        ident=c['id'];ann,defs,_=COMPACT[ident]
        if ident not in CASE_CONTEXT:errors.append('Missing parameter scope '+ident)
        if 'class="phase-location"' not in card or 'MIR：' not in card:
            errors.append('Missing MIR/PCG layer separation '+ident)
        if re.search(r'//@\s+at bb\d+\[',ann):errors.append('PCG phase inside Rust '+ident)
        if re.search(r'(generic|concrete)\s*:.*\|--',defs):
            errors.append('Conversion misrepresented as entailment '+ident)
        if 'GenericOwn T q 4' in defs+ann+c['annotated_rust']:
            errors.append('Scalar used as struct snapshot '+ident)
        if any('term-'+key not in doc.ids for key in dependencies(ann,defs)):
            errors.append('Unresolved semantic dependency '+ident)
        origins=locate(ident,ann)
        origin_count+=len(origins)
        covered={i for item in origins for i in range(item['start_line'],item['end_line']+1)}
        for number,line in enumerate(ann.splitlines(),1):
            if re.search(r'//@\s+(?:assert|requires|ensures|invariant)\b|\b(?:assert!|body_invariant!)\(',line) and number not in covered:
                errors.append('Assertion without provenance '+ident+':'+str(number))
        if card.count('class="origin-line"')!=len(origins):
            errors.append('Missing visible provenance '+ident)
        for item in origins:
            if not item['source'] or not item['reason'] or not item['detail'] or not resolve_refs(item,c):
                errors.append('Incomplete assertion provenance '+ident)
        if re.search(r'//@\s+(?:PRE|POST)\b',ann+'\n'+c['annotated_rust']):
            errors.append('Local PRE/POST '+ident)
        if re.findall(r'data-part="(\d)"',card)!=list('12345'):
            errors.append('Section order '+ident)
        definition=re.search(r'<section class="card definition".*?</section>',card,re.S)[0]
        annotation=re.search(r'<section class="card annotation".*?</section>',card,re.S)[0]
        primary,supporting=presentation_parts(ident)
        expected_definition=[legend for legend in [notation(primary),notation(supporting,primary)] if legend]
        for section, expected in [(definition,expected_definition),(annotation,[notation(ann,defs)] if notation(ann,defs) else [])]:
            legends=re.findall(r'<p class="notation">(.*?)</p>',section,re.S)
            if legends!=expected:errors.append('First-use notation '+ident)
        if 'class="status ' in card:errors.append('Presentation status badge '+ident)
        if supporting and not (definition.index('primary-rule')<definition.index('focus-reason')<definition.index('supporting-definitions')):
            errors.append('Definition presentation order '+ident)
        if 'focus-title' not in definition or 'focus-reason' not in definition:
            errors.append('Missing focused rule provenance '+ident)
        if len(defs.splitlines())>(6 if ident=='borrow-create' else 4):
            errors.append('Too many focal rule lines '+ident)
        if 'symbols-'+ident not in doc.ids:errors.append('Missing complete definition dialog '+ident)
        symbol_dialog=re.search(r'<template id="symbols-'+ident+r'">(.*?)</template>',deck.read_text(),re.S)
        if symbol_dialog:
            definition_code=re.search(r'<code>(.*?)</code>',symbol_dialog[1],re.S)[1]
            from html import unescape
            if unescape(re.sub(r'<[^>]*>','',definition_code))!=FULL_COMPACT[ident][1]:
                errors.append('Full definitions changed '+ident)
        for text in [ann,c['annotated_rust']]:
            signature=next((i for i,line in enumerate(text.splitlines()) if re.search(r'\bfn\s',line)),None)
            for i,line in enumerate(text.splitlines()):
                if re.search(r'//@\s+(requires|ensures)\b',line) and (signature is None or i>=signature):
                    errors.append('Misplaced function contract '+ident)
    if '逻辑地址模型；泛型资源由类型转换契约连接。' in deck.read_text():
        errors.append('Redundant generic footer')
    first=COMPACT['borrow-create']
    for fragment in ['I(p,v) := cell 1 p i32_val v', 'G(p,v) := GenericOwn I32 p v',
                     'Ref(s,p) := DirectMut I32 s (p,unit)', 'DirectMut T s (p,m) :=','cell 1 s ref_addr p','cell 1 s ref_meta m','Metadata(T)']:
        if fragment not in first[1]:errors.append('Incomplete inline DirectMut definition')
    first_annotated=deriv['cases'][0]['annotated_rust']
    ordered=['let mut a = 6;', '//@ assert I(loc(a),6)', 'let x = &mut a;',
             '//@ proof make_generic I32 loc(a)', '//@ assert Ref(loc(x),loc(a)) ** G(loc(a),6)']
    positions=[first_annotated.find(s) for s in ordered]
    if -1 in positions or positions!=sorted(positions):errors.append('Borrow assertion scheduling')
    # Check the semantic distinctions against the frozen backend declarations.
    vpr=(ROOT/'evidence/pcg-encoding-20261125/mutable/log/viper_program/program-check.vpr').read_text()
    direct=re.search(r'predicate p_Ref_mutable\([^\n]*\) \{(.*?)\n\}',vpr,re.S)[1]
    if 'p_Ref_mutable_addr' not in direct or 'p_Ref_mutable_metadata' not in direct or 'p_Param(' in direct:
        errors.append('Direct-reference evidence no longer matches')
    if not re.search(r'predicate p_Param\([^\n]*\)\s*\n\s*predicate',vpr):
        errors.append('Generic predicate opacity evidence changed')
    for item in TERMS.values():
        if any(dep not in TERMS for dep in item['dependencies']):errors.append('Unresolved term dependency')
    for item in presentation['cases']:
        if item['rust_lines']>12 or item['annotation_lines']>10 or item['definition_lines']>7:
            errors.append('Slide text budget '+item['id'])
    report={'errors':errors,'cases':len(deriv['cases']),'local_links_checked':count,
      'rust_and_viper_hash_pairs_checked':len(deriv['cases']),
      'rust_preserved_after_removing_annotations':not any('Annotation changed' in e for e in errors),
      'deck_sha256':hashlib.sha256(deck.read_bytes()).hexdigest(),
      'presentation_cases':len(main_cards),'syntax_highlighted_cases':sum('tok-' in c for c in main_cards),
      'assertion_and_proof_origin_notes':origin_count,
      'all_displayed_assertions_have_origins':not any('Assertion without provenance' in e for e in errors),
      'semantic_term_definitions':len(TERMS),'case_parameter_scopes':len(CASE_CONTEXT),
      'focused_rules':len(FOCUS),'notation_at_first_card':not any('First-use notation' in e for e in errors),
      'direct_mut_body_and_annotation_order_checked':not any('DirectMut' in e or 'scheduling' in e for e in errors),
      'canvas':presentation['canvas'],
      'rocq_proof_checked':False,'proof_notation':'curated explanatory DSL',
      'browser_report':'browser-checks.json'}
    (OUT/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(report,ensure_ascii=False))
    if errors:raise SystemExit(1)
if __name__=='__main__':main()
