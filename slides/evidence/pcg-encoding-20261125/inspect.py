from pathlib import Path
import json,sys
root=Path(__file__).resolve().parent
for case in sys.argv[1:]:
 d=root/case/'data'
 if not d.exists():continue
 for path in sorted(d.glob('*/pcg_data.json')):
  fn=path.parent.name
  if ':' in fn:continue
  data=json.loads(path.read_text());mir=json.loads((path.parent/'mir.json').read_text())
  print('\nCASE',case,fn)
  for bb,v in sorted(data.items()):
   node=next((n for n in mir['nodes'] if n['id']==bb),None)
   for i,s in enumerate(v['statements']):
    actions=[]
    for phase,acts in s['actions'].items():
     for a in acts:
      kind=a['action']['data']['kind'];text=str(kind['data'])
      if kind['type'] in ('AddEdge','RemoveEdge') or 'Loop(' in text or 'Unsize' in text:
       actions.append(phase+' '+kind['type']+': '+text[:200])
    if actions:
     stmt=(node['stmts']+[node['terminator']])[i]['stmt'] if node else ''
     print(bb,i,stmt[:100]);print('\n'.join(actions))
