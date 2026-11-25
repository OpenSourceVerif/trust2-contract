/* Offline, one-case-per-slide navigation. Highlighting is embedded at build time. */
(()=>{
 'use strict';
 const cases=JSON.parse(document.getElementById('case-data').textContent);
 const select=document.getElementById('case-select'),deck=document.getElementById('deck');
 const dialog=document.getElementById('evidence-dialog'),content=document.getElementById('dialog-content');
 document.addEventListener('click',e=>{
  const link=e.target.closest('.term-link');if(!link)return;
  const template=document.getElementById(link.dataset.term);if(!template)return;
  e.preventDefault();content.replaceChildren(template.content.cloneNode(true));
  document.getElementById('dialog-title').textContent='符号定义、参数与证据';
  if(!dialog.open)dialog.showModal();dialog.scrollTop=0;
 });
 let current=0;
 function fit(){deck.style.transform=`scale(${Math.min(innerWidth/1760,innerHeight/990)})`;}
 function show(id,updateHash=false,selectedStage=null){
  const i=cases.findIndex(c=>c.id===id);current=i<0?0:i;const item=cases[current];
  document.querySelectorAll('#cases>.case').forEach(n=>n.classList.toggle('active',n.id===item.id));
  document.querySelectorAll('.step').forEach(n=>n.setAttribute('aria-pressed',String(n.dataset.id===(selectedStage||item.stage))));
  select.value=item.id;document.getElementById('counter').textContent=`${current+1} / ${cases.length}`;
  document.getElementById('previous').disabled=current===0;document.getElementById('next').disabled=current===cases.length-1;
  document.getElementById('announce').textContent=item.title;document.title=item.title+' · PCG 资源规约';
  if(updateHash)history.pushState(null,'','#'+item.id);
 }
 function advance(delta){show(cases[Math.max(0,Math.min(cases.length-1,current+delta))].id,true);}
 const defaults={'stage-input':'borrow-create','stage-facts':'borrow-end','stage-pcg':'history','stage-types':'deref','stage-phases':'weaken','stage-edges':'borrow-end','stage-calls':'basic-wand','stage-casts':'unsize','stage-control':'loop-wand','stage-viper':'inhale-path','stage-verification':'remote','stage-export':'basic-wand'};
 for(const n of document.querySelectorAll('.step')){
  const activate=()=>show(defaults[n.dataset.id],true,n.dataset.id);
  n.addEventListener('click',activate);n.addEventListener('keydown',e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();activate();}});
 }
 select.addEventListener('change',()=>show(select.value,true));
 document.getElementById('previous').addEventListener('click',()=>advance(-1));
 document.getElementById('next').addEventListener('click',()=>advance(1));
 document.getElementById('print').addEventListener('click',()=>window.print());
 document.getElementById('fullscreen').addEventListener('click',async()=>{try{if(document.fullscreenElement)await document.exitFullscreen();else await document.documentElement.requestFullscreen();}catch{document.getElementById('announce').textContent='浏览器未允许全屏；可使用浏览器全屏快捷键。';}});
 document.addEventListener('keydown',e=>{if(dialog.open||e.target.closest('select,input,textarea,button,[role=button]'))return;if(['ArrowRight','PageDown'].includes(e.key)){e.preventDefault();advance(1);}if(['ArrowLeft','PageUp'].includes(e.key)){e.preventDefault();advance(-1);}});
 for(const b of document.querySelectorAll('.open-evidence'))b.addEventListener('click',()=>{
  content.replaceChildren(document.getElementById('evidence-'+b.dataset.case).content.cloneNode(true));
  document.getElementById('dialog-title').textContent='完整定义与真实证据';dialog.showModal();dialog.scrollTop=0;
 });
 for(const b of document.querySelectorAll('.zoom-graph'))b.addEventListener('click',()=>{
  const img=document.createElement('img');img.src=b.dataset.image;img.className='graph-full';img.alt='真实 PCG 图';content.replaceChildren(img);
  document.getElementById('dialog-title').textContent='PCG 图 · 原始比例';dialog.showModal();dialog.scrollTop=0;
 });
 document.getElementById('close-dialog').addEventListener('click',()=>dialog.close());
 dialog.addEventListener('click',e=>{if(e.target===dialog){const r=dialog.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)dialog.close();}});
 window.addEventListener('resize',fit);document.addEventListener('fullscreenchange',fit);
 function route(){let id='';try{id=decodeURIComponent(location.hash.slice(1));}catch{}show(id);}
 window.addEventListener('hashchange',route);fit();route();
})();
