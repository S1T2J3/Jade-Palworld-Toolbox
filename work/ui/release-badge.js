// Only the notice index and read flags are needed on the first screen.
const jadeReleases=(()=>{
  const currentVersion=document.querySelector('meta[name="pal-release-version"]').content;
  const repository=document.querySelector('meta[name="pal-release-repository"]').content;
  const releaseUrl=/^[\w.-]+\/[\w.-]+$/.test(repository)?'https://github.com/'+repository+'/releases':'';
  const desktop=!!document.querySelector('meta[name="pal-desktop"][content="standalone"]');
  const first={
    id:'release-v1.0.0',title:'Jade 幻兽帕鲁工具箱首发',publishedAt:'2026-11-09T00:00:00Z',
    body:'你可以离线查询配种组合、管理仓库、查看图鉴和探索地图。\n\n今后的公告会显示在这里。Windows 桌面版可在此检查和下载更新，已下载的版本会在下次启动时应用。'
  };
  const prefix='pal-atlas.releases.v1.',indexKey=prefix+'index',feedKey=prefix+'feed',readPrefix=prefix+'read.';
  const sessionReads=new Set();
  const safeGet=key=>{try{return localStorage.getItem(key);}catch{return null;}};
  const safeSet=(key,value)=>{try{localStorage.setItem(key,value);return true;}catch{return false;}};
  function readArray(key) {
    try {const rows=JSON.parse(safeGet(key));return Array.isArray(rows)?rows:[];}catch{return [];}
  }
  function validId(s){return typeof s==='string'&&s.length>0&&s.length<=128;}
  function loadIndex(){return [...new Set([first.id,...readArray(indexKey).filter(validId).slice(0,50)])];}
  let ids=loadIndex(), notices=null;
  let snapshot={state:'idle',currentVersion,auto:true,accessMode:'machine'};
  function isRead(id){return sessionReads.has(id)||safeGet(readPrefix+id)==='1';}
  function updateBadge(){
    const count=ids.filter(id=>!isRead(id)).length;
    const badge=document.getElementById('atlas-news-count'),link=document.getElementById('atlas-news');
    badge.hidden=count===0;badge.textContent=String(count);
    link.setAttribute('aria-label',count?'公告与更新，'+count+' 条未读公告':'公告与更新');
    link.title=link.getAttribute('aria-label');
  }
  function markRead(entries){
    for(const id of entries){
      if(!validId(id)||isRead(id))continue;
      sessionReads.add(id);safeSet(readPrefix+id,'1');
    }
    updateBadge();document.dispatchEvent(new Event('jade-notice-read'));
  }
  function normalize(rows){
    const byId=new Map([[first.id,first]]);
    for(const row of rows.slice(0,50)){
      if(!row||!validId(row.id)||typeof row.title!=='string'||!row.title.trim()||typeof row.body!=='string')continue;
      byId.set(row.id,{id:row.id,title:row.title.slice(0,240),body:row.body.slice(0,12000),
        publishedAt:typeof row.publishedAt==='string'?row.publishedAt.slice(0,64):''});
    }
    return [...byId.values()].sort((a,b)=>(Date.parse(b.publishedAt)||0)-(Date.parse(a.publishedAt)||0));
  }
  function entries(){return notices??=normalize(readArray(feedKey));}
  function receive(event){
    let data;
    try{data=JSON.parse(event.detail);}catch{return;}
    if(!data||!['idle','current','available','ready','error','unauthorized','unconfigured','busy'].includes(data.state))return;
    snapshot=data;
    if(Array.isArray(data.announcements)){
      notices=normalize(data.announcements);
      ids=notices.map(row=>row.id);
      safeSet(feedKey,JSON.stringify(notices));safeSet(indexKey,JSON.stringify(ids));updateBadge();
    }
    document.dispatchEvent(new Event('jade-release-change'));
  }
  if(desktop)document.addEventListener('pal-release-result',receive);
  window.addEventListener('storage',event=>{
    if(event.key===indexKey||event.key===feedKey||event.key===null){
      if(event.key===null)sessionReads.clear();
      ids=loadIndex();notices=null;updateBadge();
      document.dispatchEvent(new Event('jade-release-change'));
    }else if(event.key.startsWith(readPrefix)){
      if(event.newValue!=='1')sessionReads.delete(event.key.slice(readPrefix.length));
      updateBadge();document.dispatchEvent(new Event('jade-notice-read'));
    }
  });
  updateBadge();
  return {currentVersion,releaseUrl,desktop,entries,isRead,markRead,get snapshot(){return snapshot;}};
})();
