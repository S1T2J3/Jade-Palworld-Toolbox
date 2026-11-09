function initPalDex({root,data,species,index,detailData,ranchData,avatar,dex,icon,typeSet,escape,normalize,matchScore,itemIcon,listAvatars,fillPortraits}) {
  const el=id=>root.querySelector('#dx-'+id);
  const {profiles,itemNames}=index, lessons=detailData.profiles;
  const byKey=new Map(species.map(p=>[p.key,p]));
  const elements={neutral:'无',fire:'火',water:'水',grass:'草',electric:'雷',ice:'冰',ground:'地',dark:'暗',dragon:'龙'};
  const route=p=>'#paldex/'+encodeURIComponent(p.key);
  let results=[], current=null, indexY=0, lastFocused=null;
  let cards;

  for (const [key,name] of Object.entries(elements)) el('type').add(new Option(name+'属性',key));
  for (const name of Object.keys(detailData.workIcons)) el('work').add(new Option(name,name));
  function renderIndex() {
    const query=normalize(el('search').value), type=el('type').value, job=el('work').value, order=el('order').value;
    const scored=[];
    species.forEach((pal,index)=>{
      if (type && !pal.types.includes(type)) return;
      if (job && !profiles[pal.key].work.some(([j])=>j===job)) return;
      const score=matchScore(index,query) || (query && normalize(pal.group||'').includes(query)?1:0);
      if (score) scored.push({pal,index,score});
    });
    scored.sort((a,b)=>{
      if (order!=='dex') {
        const diff=profiles[b.pal.key][order]-profiles[a.pal.key][order];
        if (diff) return diff;
      }
      return (query?b.score-a.score:0) || a.index-b.index;
    });
    results=scored.map(row=>row.pal);
    el('count').textContent=results.length===species.length?species.length+' 种及形态':results.length+' / '+species.length+' 种及形态';
    if(!cards) {
      const template=document.createElement('template');
      template.innerHTML=species.map((p,i)=>`<a class="dx-card" href="${route(p)}" data-dx-key="${escape(p.key)}">${listAvatars[i]}<span><small>${escape(dex(p))}</small><strong>${escape(p.zh)}</strong><span class="dx-tags">${typeSet(p)}</span></span></a>`).join('');
      fillPortraits(template.content);cards=[...template.content.children];
    }
    el('list').replaceChildren(...scored.map(row=>cards[row.index]));
    el('empty').hidden=results.length>0;
  }
  function rememberOrigin(event) {
    const card=event.target.closest('[data-dx-key]');
    if(card && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
      lastFocused=card.dataset.dxKey;
      indexY=window.scrollY;
    }
  }
  el('list').addEventListener('click',rememberOrigin);
  el('search').addEventListener('input',renderIndex);
  for (const key of ['type','work','order']) el(key).addEventListener('change',renderIndex);
  el('reset').addEventListener('click',()=>{
    el('search').value='';el('type').value='';el('work').value='';el('order').value='dex';
    renderIndex();el('search').focus();
  });
  function section(title,body,tools='') {
    return `<section class="dx-panel"><div class="dx-panel-title"><h2>${title}</h2>${tools}</div>${body}</section>`;
  }
  function action(url,text,shape) {
    return `<a href="${url}">${shape?icon(shape):''}${text}</a>`;
  }
  function itemLink(id,extra='') {
    const src=itemIcon(id);
    return `<a class="dx-thing" href="#items/${encodeURIComponent(id)}">${src?'<img alt="" src="'+src+'">':''}<span>${escape(itemNames[id]||id)}</span>${extra}</a>`;
  }
  function dropList(level) {
    const entries=data[current.key].drops?.find(([start])=>start===level)?.[1]||[];
    return entries.map(([id,min,max,percent])=>itemLink(id,`<small>×${min===max?min:min+'–'+max} · ${percent}%</small>`)).join('');
  }
  function dropsPanel(info) {
    if (!info.drops?.length) return '';
    const choice=info.drops.length>1?`<label class="dx-drop-label">等级档<select id="dx-drop-tier">${info.drops.map(([level])=>`<option value="${level}">${level?'Lv. '+level+' 起':'基础档'}</option>`).join('')}</select></label>`:'';
    return section('普通个体掉落',`<div class="dx-thing-list" id="dx-drops">${dropList(info.drops[0][0])}</div>`,choice);
  }
  function ranchPanel(key) {
    const yields=ranchData.pals[key];
    if (!yields) return '';
    const ids=new Set(yields.tiers.flat().map(r=>r.item));
    return section('牧场产物',`<div class="dx-thing-list">${[...ids].map(id=>itemLink(id)).join('')}</div>`,action('#work/ranch/'+encodeURIComponent(key),'查看放牧产量 →'));
  }
  function renderDetail(pal) {
    const key=encodeURIComponent(pal.key), profile=profiles[pal.key], extra=data[pal.key], lesson=lessons[pal.key];
    const area=pal.types.map(t=>elements[t]).join('、');
    const links=[
      action('#spawns/'+key,'刷新位置','pin'),
      ...(extra.enemies || []).map(([id,label])=>action('#enemies/'+encodeURIComponent(id),'敌人图鉴 · '+escape(label),'enemy')),
      pal.reason?'':action('#lookup/'+key,'配种组合','branch'),
      action('#details/stats/'+key,'属性值计算','list'),
      action('#details/capture/'+key,'捕捉概率','target')
    ].join('');
    const hero=`<div class="dx-hero">${avatar(pal)}<div class="dx-identity"><p class="dx-meta">${escape(dex(pal))} · ${escape(pal.en)}</p><h1 id="dx-species-name">${escape(pal.zh)}</h1><div class="dx-tags" aria-label="${area}属性">${typeSet(pal)}</div></div><nav class="dx-actions" aria-label="${escape(pal.zh)}的相关工具">${links}</nav></div>`;
    const properties=[['生命',profile.hp],['攻击',profile.attack],['防御',profile.defense],['食量',extra.food]];
    const stats=section('基础属性',`<dl class="dx-properties">${properties.map(([label,value])=>`<div><dt>${label}</dt><dd>${Number.isFinite(value)?value:'—'}</dd></div>`).join('')}</dl>`);
    const partner=section('伙伴技能',profile.partner?`<p><b class="dx-partner">${escape(profile.partner)}</b></p><p>${escape(extra.partnerEffect||'')}</p>`:'<p>该形态暂无独立的伙伴技能记录。</p>',profile.partner?action('#details/partner/'+key,'查看伙伴技能详情 →'):'');
    const jobs=profile.work.map(([name,level])=>`<a class="dx-job" href="#work/efficiency/${key}/${encodeURIComponent(name)}"><img alt="" src="${detailData.workIcons[name]}">${escape(name)} <b>Lv. ${level}</b></a>`).join('');
    const work=section('工作适应性',jobs?'<div class="dx-job-list">'+jobs+'</div>':'<p>无工作适应性。</p>',action('#work/suitability/'+key,'查看工作排行 →')+' '+action('#work/efficiency/'+key,'计算工作效率 →'));
    const fixed=(lesson?.fixed||[]).filter(id=>detailData.passives[id]).sort((a,b)=>comparePassiveTraits(detailData.passives[a],detailData.passives[b]));
    const passive=section('固定被动',fixed.length?fixed.map(id=>`<p class="dx-passive"><b><a href="#details/passive/${key}/${encodeURIComponent(id)}">${passiveRankIcon(detailData.passives[id].rank)}${escape(detailData.passives[id].zh)}</a></b>${escape(detailData.passives[id].effect)}</p>`).join(''):'<p>无已记录的固定词条。</p>',action('#details/passive/'+key,'查看被动技能 →'));
    const moves=(lesson?.levels||[]).map(([lv,id])=>{
      const move=detailData.moves[id];
      return `<tr><td>Lv. ${lv}</td><td><a href="#details/active/${key}/${encodeURIComponent(id)}">${escape(move.zh)}</a></td><td>${typeSet({types:[move.type]})}</td><td>${move.power}</td></tr>`;
    }).join('');
    const active=section('升级习得技能',moves?`<table class="dx-skill-table"><thead><tr><th>等级</th><th>主动技能</th><th>属性</th><th>威力</th></tr></thead><tbody>${moves}</tbody></table>`:'<p>该形态没有已记录的升级习得技能。</p>',action('#details/active/'+key,'查看技能资料 →'));
    const limits=pal.reason?section('配种条件',`<p>${escape(pal.reason)}</p>`):'';
    const sellers=(extra.sellers||[]).map(([url,label])=>action(escape(url),escape(label)));
    const merchants=sellers.length?section('可能出售此帕鲁的商人','<div class="dx-thing-list">'+sellers.join(' ')+'</div>'):'';
    el('record').innerHTML=hero+`<div class="dx-section-grid"><div class="dx-stack">${stats}${partner}${work}${ranchPanel(pal.key)}${limits}${merchants}</div><div class="dx-stack">${active}${passive}${dropsPanel(extra)}</div></div>`;
    const group=results.includes(pal)?results:species, i=group.indexOf(pal);
    adjacent('prev',group[i-1]);adjacent('next',group[i+1]);
  }
  function adjacent(id,pal) {
    const link=el(id);
    if(pal) {
      link.href=route(pal);link.removeAttribute('aria-disabled');
      link.title=pal.zh;link.setAttribute('aria-label',(id==='prev'?'上一只，':'下一只，')+pal.zh);
    } else {
      link.removeAttribute('href');link.removeAttribute('title');link.removeAttribute('aria-label');link.setAttribute('aria-disabled','true');
    }
  }
  el('record').addEventListener('change',event=>{
    if(event.target.id==='dx-drop-tier') el('drops').innerHTML=dropList(Number(event.target.value));
  });
  return {show(key) {
    const previous=current;
    current=byKey.get(key)||null;
    el('index').hidden=Boolean(key);el('detail').hidden=!key;
    root.setAttribute('aria-labelledby',key?'dx-species-name':'dx-title');
    if (current) renderDetail(current);
    else if (key) {
      el('record').innerHTML='<div class="dx-panel"><h1 id="dx-species-name">找不到这只帕鲁</h1><p>请返回图鉴按名称或编号查找。</p></div>';
      adjacent('prev',null);adjacent('next',null);
    } else if(!cards) renderIndex();
    else if(previous && lastFocused) {
      requestAnimationFrame(()=>{
        if(root.hidden||el('index').hidden) return;
        el('list').querySelector('[data-dx-key="'+CSS.escape(lastFocused)+'"]')?.focus({preventScroll:true});
        window.scrollTo({top:indexY,behavior:'instant'});
      });
    }
  }};
}
