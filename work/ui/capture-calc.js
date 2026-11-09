function estimateCapture({power,level,hp,coefficient,world,status=0,blocked=false}) {
  if(blocked || hp<=0 || coefficient<=0 || world<=0) return {reticle:0,chance:0};
  const advantage=Math.max(0,Math.min(1,(power-level+50)/99));
  const score=advantage*Math.pow(1.3,-hp/100);
  const base=score<0.5 ? 256*score**9 : 1-(2-2*score)**9/2;
  const roll=Math.min(1,Math.max(0,base*(1+status)*coefficient*world));
  return {reticle:roll**(5/12),chance:Math.cbrt(roll)};
}

function initCaptureCalc({root,species,itemIcon,escape,spawnSites,data}) {
  const field=name=>root.querySelector('#cc-'+name);
  const number=name=>field(name).valueAsNumber;
  const modules=new Map(data.modules.map(m=>[m.id,m.power]));
  const pals=new Map(species.map(p=>[p.key,p]));
  const spheres=field('sphere'), regions=field('encounter'), comparison=field('compare'), output=field('output');
  let current;
  for(const ball of data.spheres) spheres.add(new Option(ball.name,ball.id));
  for(const mod of data.modules) field('module').add(new Option(mod.name+'（+'+mod.power+'）',mod.id));
  function percent(value) {
    if(value===0 || value===1) return (value*100)+'%';
    if(value<0.0001) return '&lt;0.01%';
    if(value>0.9999) return '&gt;99.99%';
    return Number((value*100).toFixed(2))+'%';
  }
  function refresh() {
    const statusId=field('status').value!=='0'?field('status').value:field('poison').checked?'poison':'';
    field('status-reference').href='#guide/status'+(statusId?'/'+encodeURIComponent(statusId):'');
    const invalid=[...root.querySelectorAll('input[type="number"]')].find(el=>!el.validity.valid);
    if(invalid) {
      output.innerHTML='<p class="cc-error">请填写有效的'+escape(invalid.labels[0].firstChild.textContent)+'（'+invalid.min+'～'+invalid.max+'）。</p>';
      comparison.replaceChildren();return;
    }
    const [normal,alpha]=data.profiles[current];
    const coefficient=regions.value==='alpha'?alpha:normal;
    const hp=number('hp'), level=number('level'), world=number('world'), n=number('throws');
    const status=(field('status').value!=='0'?0.30:0)+(field('poison').checked?0.30:0)+(field('trap').checked?0.50:0);
    const bonus=number('effigy')/2+(modules.get(field('module').value)||0)+number('partner')+(field('critical').checked?2:0);
    const raidEgg=data.raidEggOnly.includes(current);
    const blocked=current==='WorldTreeDragon' || raidEgg || regions.value==='blocked';
    const rows=data.spheres.map(ball=>{
      const power=ball.power+bonus;
      return {...ball,power,...estimateCapture({power,level,hp,coefficient,world,status,blocked})};
    });
    const selected=rows.find(r=>r.id===spheres.value);
    const cumulative=selected.chance===1?1:Math.min(1-Number.EPSILON,-Math.expm1(n*Math.log1p(-selected.chance)));
    let note=escape(pals.get(current).zh)+' · '+escape(selected.name)+' · 总捕获力 '+selected.power+'。多球概率按每次投球条件相同计算。';
    if(current==='WorldTreeDragon') note='枯星龙属于无法用帕鲁球捕捉的塔主。';
    else if(raidEgg) {
      const records=data.enemiesByPal?.[current]||[];
      const links=records.map(([id,label])=>'<a href="#enemies/'+encodeURIComponent(id)+'">'+escape(label)+'</a>');
      note='常规世界中的'+escape(pals.get(current).zh)+'为团体战头目，无法投球捕捉。击败后可从奖励帕鲁蛋孵出。 '+
        (links.length?'敌人图鉴 · '+links.join('、'):'<a href="https://palpedia.com/guides/raid-bosses" target="_blank" rel="noopener noreferrer">团体战资料 ↗</a>');
    }
    else if(blocked) note='这场战斗禁止捕捉。';
    else if(hp===0) note='目标已倒下，无法捕捉。';
    else if(world===0) note='当前世界捕捉倍率为 0。';
    else if(!spawnSites[current]?.length) note+=' 手册暂无野外刷新坐标，数值按当前选择的可捕捉遭遇估算。';
    output.innerHTML='<div class="cc-summary"><div><small>单球成功率（估算）</small><b>'+percent(selected.chance)+'</b></div>'+
      '<div><small>准星预估</small><strong>'+percent(selected.reticle)+'</strong></div><div><small>'+n+' 球内至少成功一次</small><strong>'+percent(cumulative)+'</strong></div></div><p class="cc-note">'+note+'</p>';
    comparison.innerHTML=rows.map(r=>'<button class="cc-row" type="button" data-ball="'+escape(r.id)+'" aria-pressed="'+(r.id===selected.id)+'">'+
      '<span><img src="'+escape(itemIcon(r.id))+'" alt="">'+escape(r.name)+'</span><span>'+percent(r.reticle)+'</span><b>'+percent(r.chance)+'</b></button>').join('');
  }
  root.addEventListener('input',refresh);
  comparison.addEventListener('click',event=>{
    const button=event.target.closest('button[data-ball]');
    if(!button) return;
    spheres.value=button.dataset.ball;refresh();
    comparison.querySelector('[aria-pressed="true"]').focus({preventScroll:true});
  });
  return {show(key) {
    current=key;
    const hasAlpha=data.profiles[key][1]!==null;
    regions.querySelector('[value="alpha"]').disabled=!hasAlpha;
    if(!hasAlpha && regions.value==='alpha') regions.value='wild';
    refresh();
  }};
}
