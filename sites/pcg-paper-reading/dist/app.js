const sidebar=document.querySelector('.sidebar');
const menu=document.querySelector('#menu');
menu.addEventListener('click',()=>{const opened=sidebar.classList.toggle('open');menu.setAttribute('aria-expanded',String(opened));});
const links=[...document.querySelectorAll('nav[aria-label="章节目录"] a')];
sidebar.querySelectorAll('a').forEach(a=>a.addEventListener('click',()=>{sidebar.classList.remove('open');menu.setAttribute('aria-expanded','false');}));
document.addEventListener('keydown',e=>{if(e.key==='Escape'&&sidebar.classList.contains('open')){sidebar.classList.remove('open');menu.setAttribute('aria-expanded','false');menu.focus();}});
const chapters=[...document.querySelectorAll('.chapter')];
let queued=false;
function updateNav(){const active=chapters.filter(s=>s.getBoundingClientRect().top<window.innerHeight*.4).at(-1)||chapters[0];links.forEach(a=>{const yes=a.hash==='#'+active.id;a.classList.toggle('active',yes);if(yes)a.setAttribute('aria-current','location');else a.removeAttribute('aria-current');});queued=false;}
window.addEventListener('scroll',()=>{if(!queued){queued=true;requestAnimationFrame(updateNav)}},{passive:true});updateNav();
const modal=document.querySelector('#figure-dialog');
document.querySelectorAll('.figure-button').forEach(b=>b.addEventListener('click',()=>{const img=b.querySelector('img');modal.querySelector('img').src=img.src;modal.querySelector('img').alt=img.alt;modal.querySelector('#dialog-title').textContent=img.alt;modal.showModal();}));
modal.querySelector('button').addEventListener('click',()=>modal.close());
modal.addEventListener('click',e=>{if(e.target===modal){const r=modal.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)modal.close();}});
