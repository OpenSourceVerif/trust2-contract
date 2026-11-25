/* Offline workflow navigation; every example and source excerpt is static HTML. */
(()=>{
 'use strict';
 const data=JSON.parse(document.getElementById('workflow-data').textContent);
 const stages=new Map(data.stages.map(s=>[s.id,s]));
 const cases=new Map(data.cases.map(c=>[c.id,c]));
 const caseNodes=new Map([...document.querySelectorAll('.case-record')].map(n=>[n.id,n]));
 const summaries=new Map([...document.querySelectorAll('.stage-summary')].map(n=>[n.id,n]));
 const references=new Map([...document.querySelectorAll('.reference')].map(n=>[n.id,n]));
 const wrappers=new Map();
 for(const [id,node] of references){
  const fold=document.createElement('details');fold.className='ref-fold';
  const summary=document.createElement('summary');summary.textContent=node.querySelector('h3').textContent;
  node.before(fold);fold.append(summary,node);wrappers.set(id,fold);
 }
 const panel=document.querySelector('.detail-panel');
 const caseSelect=document.getElementById('case-select');
 const stageSelect=document.getElementById('stage-select');
 let stageId='',caseId='';
 document.documentElement.classList.add('js');
 function reset(){
  for(const n of document.querySelectorAll('#summary-host>.stage-summary'))document.getElementById('summary-library').append(n);
  for(const n of document.querySelectorAll('#case-host>.case-record'))document.getElementById('case-library').append(n);
  for(const n of document.querySelectorAll('#references-host>.ref-fold'))document.getElementById('reference-library').append(n);
 }
 function show(nextStage,nextCase,hash,scroll=true){
  const stage=stages.get(nextStage);if(!stage)return;
  reset();stageId=nextStage;caseId=stage.cases.includes(nextCase)?nextCase:stage.default;
  const item=cases.get(caseId);
  document.getElementById('summary-host').append(summaries.get(stageId));
  document.getElementById('case-host').append(caseNodes.get(caseId));
  document.querySelectorAll('.step').forEach(n=>n.setAttribute('aria-pressed',String(n.dataset.id===stageId)));
  stageSelect.value=stageId;caseSelect.replaceChildren();
  for(const id of stage.cases){const option=document.createElement('option');option.value=id;option.textContent=cases.get(id).title+' · '+(cases.get(id).status===0?'通过':'失败');caseSelect.append(option);}
  caseSelect.value=caseId;document.getElementById('case-count').textContent=`${stage.cases.length} 个案例`;
  const ids=[...new Set([...item.rules.map(r=>'rule-'+r),...stage.refs])];
  for(const id of ids){const wrapper=wrappers.get(id);wrapper.open=false;document.getElementById('references-host').append(wrapper);}
  if(ids.length)wrappers.get(ids[0]).open=true;
  const index=data.stages.findIndex(s=>s.id===stageId);
  document.getElementById('counter').textContent=`${index+1} / ${data.stages.length}`;
  document.getElementById('previous').disabled=index===0;
  document.getElementById('next').disabled=index===data.stages.length-1;
  document.getElementById('announcement').textContent=stage.title+'，案例：'+item.title;
  document.title=stage.title+' · Prusti → PCG → Viper';
  if(hash)history.replaceState(null,'','#'+hash);
  if(scroll)panel.scrollTop=0;
 }
 function route(){
  let id;try{id=decodeURIComponent(location.hash.slice(1));}catch{id='';}
  if(stages.has(id)){show(id,stages.get(id).default);return;}
  const cid=cases.has(id)?id:id.endsWith('-viper')&&cases.has(id.slice(0,-6))?id.slice(0,-6):null;
  if(cid){
   const stage=stages.get(stageId);
   show(stage?.cases.includes(cid)?stageId:data.canonical[cid],cid);
   if(id.endsWith('-viper'))document.getElementById(id).scrollIntoView({block:'start'});
   return;
  }
  if(references.has(id)){
   let target=stages.get(stageId);
   const fits=s=>s&&(s.refs.includes(id)||s.cases.some(c=>cases.get(c).rules.some(r=>'rule-'+r===id)));
   if(!fits(target))target=data.stages.find(fits);
   if(target){
    const cid=target.refs.includes(id)?target.default:target.cases.find(c=>cases.get(c).rules.some(r=>'rule-'+r===id));
    show(target.id,cid);
    const fold=wrappers.get(id);fold.open=true;fold.scrollIntoView({block:'start'});return;
   }
  }
  show('stage-input','call-edge');
 }
 function selectStage(id){show(id,stages.get(id).default,id);}
 for(const node of document.querySelectorAll('.step')){
  node.addEventListener('click',()=>selectStage(node.dataset.id));
  node.addEventListener('keydown',e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();selectStage(node.dataset.id);}});
 }
 stageSelect.addEventListener('change',()=>selectStage(stageSelect.value));
 caseSelect.addEventListener('change',()=>show(stageId,caseSelect.value,caseSelect.value));
 function step(offset){const i=data.stages.findIndex(s=>s.id===stageId);const s=data.stages[i+offset];if(s)selectStage(s.id);}
 document.getElementById('previous').addEventListener('click',()=>step(-1));
 document.getElementById('next').addEventListener('click',()=>step(1));
 window.addEventListener('hashchange',route);
 document.addEventListener('click',e=>{
  const a=e.target.closest('a[href^="#"]');if(!a)return;
  e.preventDefault();history.pushState(null,'',a.getAttribute('href'));route();
 });
 const toggle=document.getElementById('toggle-flow');
 toggle.addEventListener('click',()=>{const collapsed=document.body.classList.toggle('flow-collapsed');toggle.textContent=collapsed?'展开流程':'收起流程';toggle.setAttribute('aria-expanded',String(!collapsed));});
 document.getElementById('print').addEventListener('click',()=>window.print());
 const foldsBeforePrint=new Map();
 window.addEventListener('beforeprint',()=>{for(const fold of wrappers.values()){foldsBeforePrint.set(fold,fold.open);fold.open=true;}});
 window.addEventListener('afterprint',()=>{for(const [fold,open] of foldsBeforePrint)fold.open=open;foldsBeforePrint.clear();});
 route();
})();
