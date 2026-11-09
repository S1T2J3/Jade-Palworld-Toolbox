function calculatePalStats(profile, config, passive) {
  const combat=['hp','attack','defense'];
  const panel={}, expedition={};
  const f32=Math.fround, stars=f32(1+0.05*config.stars);
  combat.forEach((stat,i)=>{
    // The native coefficients have one decimal place. Apply the integer
    // percentage factors before dividing to avoid e.g. 97.99999 -> 97.
    const baseTenths=Math.round(profile.growth[i]*(config.awaken?11:10)+profile.trust[i]*10*config.trust);
    const scaled=baseTenths*config.level*(1000+3*config.iv[stat]);
    const base=stat==='hp'
      ? 500+5*config.level+Math.floor(scaled/20000)
      : (stat==='attack'?100:50)+Math.floor(scaled*3/400000);
    const souls=f32(1+0.03*config.souls[stat]);
    // A one-star, level-10 Lamball with zero IVs has 944 HP in the
    // published game sample. Integer percentage multiplication yields 945.
    const train=value=>Math.floor(f32(f32(value*souls)*stars));
    expedition[stat]=train(base);
    panel[stat]=Math.max(0,train(f32(base*f32(1+(passive[stat]||0)/100))));
  });
  const work=(70+7*config.stars)*(100+3*config.souls.work)*(100+(passive.work||0));
  panel.work=Math.max(0,Math.floor(work/10000));
  const factor=(config.stars+1)**2;
  const power=(Math.floor(expedition.hp/5)+expedition.attack+expedition.defense)*factor;
  return {panel,expedition,power,factor};
}

function initPalStatCalc({root,data,detailData,escape}) {
  const host=root||document.getElementById('pal-stat-calc');
  const get=id=>host.querySelector('#sc-'+id);
  const attr={hp:'生命',attack:'攻击',defense:'防御',work:'工作速度'};
  const digits=new Intl.NumberFormat('zh-CN');
  const names=detailData.passives;
  let current='SheepBall';
  const tags=Object.keys(data.modifiers).sort((a,b)=>comparePassiveTraits(names[a],names[b]));
  const slots=[];
  for (let i=0;i<=4;i++) get('stars').add(new Option(i+' 星',i));
  for (let i=0;i<=10;i++) get('trust').add(new Option(i+' 级',i));

  const inputFor=(stat,kind,max)=>{
    const input=document.createElement('input');
    input.id='sc-'+kind+'-'+stat;
    input.type='number';input.required=true;
    input.min=0;input.max=max;input.step=1;input.value=0;
    input.setAttribute('aria-label',attr[stat]+(kind==='iv'?'个体值':'帕鲁之魂强化等级'));
    return input;
  };
  for (const [stat,name] of Object.entries(attr)) {
    const row=get('rows').insertRow();
    const heading=document.createElement('th');
    heading.scope='row';heading.textContent=name;row.append(heading);
    const iv=row.insertCell();
    if (stat==='work') iv.textContent='—';
    else iv.append(inputFor(stat,'iv',100));
    row.insertCell().append(inputFor(stat,'souls',20));
    row.insertCell().id='sc-panel-'+stat;
    const expedition=row.insertCell();
    expedition.id='sc-expedition-'+stat;expedition.textContent='—';
  }
  const picker=document.createElement('dialog');
  picker.id='sc-trait-picker';picker.setAttribute('aria-labelledby','sc-picker-title');
  picker.innerHTML='<div class="sc-picker-heading"><h3 id="sc-picker-title"></h3><button type="button" data-sc-close>关闭</button></div><div class="sc-picker-choices"></div>';
  host.append(picker);
  const choices=picker.querySelector('.sc-picker-choices');
  let editingSlot=null;
  function openPicker(control) {
    if(picker.open) return;
    editingSlot=control;
    picker.querySelector('h3').textContent='选择词条 '+(slots.indexOf(control)+1);
    choices.innerHTML=Array.from(control.options,option=>{
      const trait=names[option.value];
      return '<button type="button" data-sc-trait="'+escape(option.value)+'" aria-pressed="'+option.selected+'"'+
        (option.disabled?' disabled':'')+(trait?' title="'+escape(trait.effect)+'"':'')+'>'+
        passiveRankIcon(trait?.rank)+escape(trait?.zh||'无词条')+'</button>';
    }).join('');
    picker.showModal();choices.querySelector('[aria-pressed="true"]')?.focus();
  }
  picker.addEventListener('click',event=>{
    const chosen=event.target.closest('[data-sc-trait]');
    if(chosen && !chosen.disabled && editingSlot) {
      editingSlot.value=chosen.dataset.scTrait;
      editingSlot.dispatchEvent(new Event('change',{bubbles:true}));
      picker.close();
    } else if(event.target.closest('[data-sc-close]')) picker.close();
    else if(event.target===picker) {
      const bounds=picker.getBoundingClientRect();
      if(event.clientX<bounds.left||event.clientX>bounds.right||event.clientY<bounds.top||event.clientY>bounds.bottom) picker.close();
    }
  });
  window.addEventListener('hashchange',()=>{if(picker.open) picker.close();});
  for (let i=0;i<4;i++) {
    const control=document.createElement('select');
    control.setAttribute('aria-haspopup','dialog');control.setAttribute('aria-controls',picker.id);
    control.add(new Option('无词条',''));
    for (const key of tags) {
      const option=new Option(names[key].zh,key);
      option.title=names[key].effect;control.add(option);
    }
    const label=document.createElement('label');
    const field=document.createElement('span'),emblem=document.createElement('span');
    field.className='sc-trait-control';emblem.setAttribute('aria-hidden','true');
    control.setAttribute('aria-label','词条 '+(i+1));
    field.append(emblem,control);label.textContent='词条 '+(i+1);label.append(field);
    control.addEventListener('pointerdown',event=>{
      if(event.button!==0) return;
      event.preventDefault();control.focus();openPicker(control);
    });
    control.addEventListener('click',event=>{event.preventDefault();openPicker(control);});
    control.addEventListener('keydown',event=>{
      if(event.key===' '||event.key==='Enter'||event.key==='F4'||event.key==='ArrowDown'&&event.altKey) {
        event.preventDefault();openPicker(control);
      }
    });
    get('passives').append(label);slots.push(control);
  }

  function render() {
    const chosen=slots.map(s=>s.value).filter(Boolean);
    for (const select of slots) {
      const preview=select.previousElementSibling;
      if(preview.dataset.trait!==select.value){
        preview.dataset.trait=select.value;
        preview.innerHTML=passiveRankIcon(names[select.value]?.rank);
      }
      for (const option of select.options) {
        option.disabled=Boolean(option.value && option.value!==select.value && chosen.includes(option.value));
      }
    }
    get('conditional').hidden=!chosen.includes('MutationPal_Mutant');
    const invalid=host.querySelector('input[type="number"]:invalid');
    const unavailable=current==='WorldTreeDragon';
    const status=get('status');
    status.hidden=!invalid&&!unavailable;
    status.textContent=invalid
      ? '请输入范围内的整数（等级 1–80，个体值 0–100，魂强化 0–20）。'
      : unavailable?'枯星龙为无法捕获的塔主。面板仅用于查看种族参数，无法派去远征。':'';
    if (invalid) {
      for (const key of Object.keys(attr)) {
        get('panel-'+key).textContent='—';get('expedition-'+key).textContent='—';
      }
      get('power-value').textContent='—';get('power-detail').textContent='';
      return;
    }
    const grown={};
    for (const key of chosen) {
      for (const [stat,value] of Object.entries(data.modifiers[key])) grown[stat]=(grown[stat]||0)+value;
    }
    const iv={}, souls={};
    for (const stat of Object.keys(attr)) {
      if (stat!=='work') iv[stat]=get('iv-'+stat).valueAsNumber;
      souls[stat]=get('souls-'+stat).valueAsNumber;
    }
    const config={iv,souls,level:get('level').valueAsNumber,stars:Number(get('stars').value),trust:Number(get('trust').value),awaken:get('awaken').checked};
    const profile=data.profiles[current][get('alpha').checked?'alpha':'normal'];
    const {panel,expedition,power,factor}=calculatePalStats(profile,config,grown);
    for (const stat of Object.keys(attr)) {
      get('panel-'+stat).textContent=digits.format(panel[stat]);
      get('expedition-'+stat).textContent=unavailable||stat==='work'?'—':digits.format(expedition[stat]);
    }
    get('power-value').textContent=unavailable?'—':digits.format(power);
    get('power-detail').textContent=unavailable?'无法派遣':'生命折算 '+digits.format(Math.floor(expedition.hp/5))+' ＋ 攻击 '+digits.format(expedition.attack)+' ＋ 防御 '+digits.format(expedition.defense)+'，星级倍率 ×'+factor;
  }
  function setTraining(maxed) {
    get('level').value=maxed?80:1;
    get('stars').value=maxed?4:0;
    get('trust').value=maxed?10:0;
    for (const key of Object.keys(attr)) {
      if (key!=='work') get('iv-'+key).value=maxed?100:0;
      get('souls-'+key).value=maxed?20:0;
    }
    if (!maxed) {
      for (const select of slots) select.value='';
      get('alpha').checked=false;get('awaken').checked=false;
    }
    render();
  }
  get('max').addEventListener('click',()=>setTraining(true));
  get('reset').addEventListener('click',()=>setTraining(false));
  host.addEventListener('input',render);
  host.addEventListener('change',render);
  return {
    show(key) {
      current=Object.hasOwn(data.profiles,key)?key:'SheepBall';
      render();
    }
  };
}
