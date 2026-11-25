/* Usage: node this-file <playwright-package> <output-directory> <base-url> <browser-executable> */
const {chromium}=require(process.argv[2]);
const fs=require('fs'),path=require('path');
(async()=>{
 const out=process.argv[3],base=process.argv[4];
 const browser=await chromium.launch({executablePath:process.argv[5],headless:true});
 const page=await browser.newPage({viewport:{width:1760,height:990},deviceScaleFactor:1});
 const errors=[],layouts=[];
 page.on('pageerror',e=>errors.push(e.message));
 page.on('response',r=>{if(r.status()>=400)errors.push(r.status()+' '+r.url());});
 await page.goto(base+'/pcg-resource-contracts.html');
 const cases=await page.locator('#case-data').evaluate(n=>JSON.parse(n.textContent));
 for(const vp of [{width:1920,height:1080},{width:1440,height:900},{width:1280,height:720}]){
  await page.setViewportSize(vp);
  for(const c of cases){
   await page.selectOption('#case-select',c.id);
   const row=await page.evaluate(()=>{
    const active=document.querySelector('#cases>.case.active');
    const r=document.getElementById('deck').getBoundingClientRect();
    return {id:active.id,width:innerWidth,height:innerHeight,
     fits:r.left>=-1&&r.top>=-1&&r.right<=innerWidth+1&&r.bottom<=innerHeight+1,
     cards:[...active.querySelectorAll('.card')].map(n=>({name:n.className,overflow:n.scrollHeight>n.clientHeight+2})),
     highlighted:active.querySelectorAll('pre .tok-keyword').length>0,
     parts:[...active.querySelectorAll('[data-part]')].map(n=>n.dataset.part).sort().join('')};
   });
   if(!row.fits||!row.highlighted||row.parts!=='12345'||row.cards.some(c=>c.overflow))errors.push(JSON.stringify(row));
   layouts.push(row);
  }
 }
 await page.setViewportSize({width:1760,height:990});
 await page.selectOption('#case-select','borrow-create');
 await page.locator('.case.active .definition a[data-term="term-DirectMut"]').first().click();
 if(!await page.locator('#dialog-content').getByText('DirectMut：引用槽自身的资源',{exact:true}).count())errors.push('DirectMut definition');
 if(!(await page.locator('#dialog-content').textContent()).includes('不拥有 p 处的目标内容'))errors.push('DirectMut referent distinction');
 await page.keyboard.press('Escape');
 await page.selectOption('#case-select','deref');
 await page.locator('.case.active .phase-location .term-link').click();
 if(!(await page.locator('#dialog-content').textContent()).includes('MIR 位置 × PCG 分析阶段'))errors.push('MIR/PCG distinction');
 await page.keyboard.press('Escape');
 for(const c of cases){
  await page.selectOption('#case-select',c.id);
  await page.waitForFunction(()=>{const i=document.querySelector('.case.active img');return i.complete&&i.naturalWidth>0});
  await page.locator('.case.active .open-evidence').click();
  if(!await page.locator('#evidence-dialog').isVisible())errors.push('Evidence dialog '+c.id);
  if(!await page.locator('#dialog-content .sources').count())errors.push('Sources '+c.id);
  await page.locator('#close-dialog').click();
 }
 await page.selectOption('#case-select','basic-wand');await page.locator('.case.active .zoom-graph').click();
 if(!await page.locator('#dialog-content .graph-full').isVisible())errors.push('Graph zoom');
 await page.keyboard.press('Escape');
 await page.locator('body').click({position:{x:1,y:1}});await page.keyboard.press('ArrowRight');
 if(await page.locator('#case-select').inputValue()!=='call-edge')errors.push('Keyboard next');
 for(const id of ['stage-facts','stage-calls','stage-control']){
  const n=page.locator(`.step[data-id="${id}"]`);await n.click();
  if(await n.getAttribute('aria-pressed')!=='true')errors.push('Flow '+id);
 }
 for(const id of ['basic-wand','remote','loop-wand','weaken']){
  await page.selectOption('#case-select',id);await page.screenshot({path:path.join(out,'slide-'+id+'.png')});
 }
 await page.goto(base+'/pcg-resource-contracts.html#basic-wand');
 if(await page.locator('#case-select').inputValue()!=='basic-wand')errors.push('Deep link');
 await page.emulateMedia({media:'print'});
 if(await page.locator('#cases>.case:visible').count()!==cases.length)errors.push('Print cases');
 await browser.close();
 fs.writeFileSync(path.join(out,'browser-checks.json'),JSON.stringify({errors,cases:cases.length,layouts,syntax_highlighting:true},null,2));
 console.log(JSON.stringify({errors,cases:cases.length,layouts:layouts.length}));
 if(errors.length)process.exitCode=1;
})().catch(e=>{console.error(e);process.exit(1)});
