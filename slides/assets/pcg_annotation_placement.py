"""Attach presentation comments to matching Rust statements without duplicating effects."""
import re

def annotate(rust, compact):
    source=rust.splitlines();inserts={};contracts=[];pending=[];previous=None;cursor=0
    def attach(index,comments):
        if comments:
            previous_line=source[index-1] if index else ''
            indent=len(previous_line)-len(previous_line.lstrip())
            if previous_line.rstrip().endswith('{'):indent+=4
            inserts.setdefault(index,[]).extend(' '*max(4,indent)+c.strip() for c in comments)
    for raw in compact.splitlines():
        line=raw.strip()
        if re.match(r'//@\s+(requires|ensures)\b',line):
            contracts.append(line);continue
        if line.startswith('//@'):
            pending.append(line);continue
        if not line or line.startswith('//'):continue
        # Inline encoder comments follow the corresponding original statement.
        code,sep,inline=line.partition('//@')
        code=code.split('//',1)[0].strip()
        match=next((i for i in range(cursor,len(source)) if source[i].strip()==code),None)
        if match is None:continue  # compact signature formatting / omitted body
        attach(previous+1 if previous is not None else match,pending);pending=[]
        previous=match;cursor=match+1
        if sep:pending.append('//@'+inline)
    if pending:
        assert previous is not None,'No source anchor for comments'
        attach(previous+1,pending)
    if contracts:
        fn=next(i for i,line in enumerate(source) if re.search(r'\bfn\s',line))
        inserts.setdefault(fn,[])[:0]=contracts
    out=[]
    for i,line in enumerate(source):out.extend(inserts.get(i,[]));out.append(line)
    out.extend(inserts.get(len(source),[]))
    assert '\n'.join(l for l in out if not l.lstrip().startswith('//@'))==rust
    return out
