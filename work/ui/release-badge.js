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
  const popupSettingKey=prefix+'popup.enabled',shownPrefix=prefix+'popup.shown.';
  const installSignalKey=prefix+'install.ready',installShownPrefix=prefix+'install.shown.';
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
  let popupsEnabled=safeGet(popupSettingKey)!=='0',popupShown=false,waiting=false,scheduled=false;
  let popupId=null;
  let installVersion='',shownInstall='',presentInstall=null;
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
      byId.set(row.id,{id:row.id,title:row.title.slice(0,240),body:row.body.slice(0,12000),popup:row.popup===true,
        publishedAt:typeof row.publishedAt==='string'?row.publishedAt.slice(0,64):''});
    }
    return [...byId.values()].sort((a,b)=>(Date.parse(b.publishedAt)||0)-(Date.parse(a.publishedAt)||0));
  }
  function entries(){return notices??=normalize(readArray(feedKey));}
  function pendingPopup(){return entries().find(row=>row.popup&&!isRead(row.id)&&safeGet(shownPrefix+row.id)!=='1');}
  function pendingInstall(){
    return installVersion && installVersion!==shownInstall && safeGet(installShownPrefix+installVersion)!=='1';
  }
  function queueInstall(version,broadcast){
    if(!desktop||typeof version!=='string'||!/^\d+(?:\.\d+){2,3}$/.test(version)||version===currentVersion)return;
    installVersion=version;
    if(!pendingInstall())return;
    if(broadcast)safeSet(installSignalKey,JSON.stringify({version,time:Date.now()}));
    if(!presentInstall)document.dispatchEvent(new Event('jade-load-install'));
    schedulePopup();
  }
  function setInstallPrompt(callback){presentInstall=callback;if(pendingInstall())schedulePopup();}
  function showPendingPopup(){
    waiting=false;
    const install=pendingInstall();
    const row=!install&&!popupShown&&popupsEnabled?pendingPopup():null;
    if(!install&&!row)return;
    const focused=document.activeElement;
    if(document.hidden||!document.hasFocus()||document.querySelector('dialog[open]')||focused?.isContentEditable||
      focused?.matches('textarea,select,input:not([type="checkbox"]):not([type="radio"]):not([type="button"]):not([type="submit"])')){
      waiting=true;return;
    }
    if(install){
      if(presentInstall&&presentInstall(installVersion)){
        shownInstall=installVersion;
        safeSet(installShownPrefix+installVersion,'1');
      }
      return;
    }
    const dialog=document.getElementById('notice-popup');
    if(!dialog?.showModal)return;
    document.getElementById('notice-popup-title').textContent=row.title;
    document.getElementById('notice-popup-body').textContent=row.body;
    const date=document.getElementById('notice-popup-date'), time=Date.parse(row.publishedAt);
    date.hidden=!Number.isFinite(time);
    if(!date.hidden){
      date.dateTime=row.publishedAt;
      date.textContent=new Date(time).toLocaleDateString('zh-CN',{year:'numeric',month:'long',day:'numeric'});
    }
    dialog.showModal();
    popupShown=true;popupId=row.id;
    safeSet(shownPrefix+row.id,'1');
    dialog.addEventListener('close',()=>{
      popupId=null;
      markRead([row.id]);
    },{once:true});
  }
  function schedulePopup(){
    if(scheduled||document.readyState!=='complete'||(!pendingInstall()&&(popupShown||!popupsEnabled)))return;
    scheduled=true;
    const check=async()=>{
      try {
        if(!pendingInstall()&&!pendingPopup())return;
        // Serialize the read/show receipt across windows sharing the same profile.
        if(navigator.locks)await navigator.locks.request(prefix+'popup',showPendingPopup);
        else showPendingPopup();
      } catch {showPendingPopup();}
      finally {scheduled=false;}
    };
    if(window.requestIdleCallback)window.requestIdleCallback(check,{timeout:600});
    else window.setTimeout(check,80);
  }
  function preferenceChanged(){
    if(popupsEnabled)schedulePopup();
    else if(popupId)document.getElementById('notice-popup').close();
    document.dispatchEvent(new Event('jade-popup-setting'));
  }
  function setPopupsEnabled(value){
    popupsEnabled=value===true;
    safeSet(popupSettingKey,popupsEnabled?'1':'0');
    preferenceChanged();
  }
  const retryIfWaiting=()=>{if(waiting)schedulePopup();};
  document.addEventListener('close',retryIfWaiting,true);
  document.addEventListener('focusout',retryIfWaiting);
  document.addEventListener('visibilitychange',retryIfWaiting);
  window.addEventListener('focus',retryIfWaiting);
  window.addEventListener('load',schedulePopup,{once:true});
  schedulePopup();
  function receive(event){
    let data;
    try{data=JSON.parse(event.detail);}catch{return;}
    if(!data||!['idle','current','available','ready','restarting','error','unauthorized','unconfigured','busy'].includes(data.state))return;
    // A transport error may carry only state and message. Keep the last
    // confirmed preferences and versions until a full native reply arrives.
    snapshot=data.currentVersion?data:{...snapshot,...data};
    if(data.state==='ready'&&(data.action==='download'||(data.action==='auto'&&data.auto===true))){
      queueInstall(data.readyVersion,true);
    }
    if(Array.isArray(data.announcements)){
      notices=normalize(data.announcements);
      ids=notices.map(row=>row.id);
      safeSet(feedKey,JSON.stringify(notices));safeSet(indexKey,JSON.stringify(ids));updateBadge();
      schedulePopup();
    }
    document.dispatchEvent(new Event('jade-release-change'));
  }
  if(desktop)document.addEventListener('pal-release-result',receive);
  window.addEventListener('storage',event=>{
    if(event.key===installSignalKey&&event.newValue){
      try{queueInstall(JSON.parse(event.newValue).version,false);}catch{}
      return;
    }
    if(event.key===popupSettingKey||event.key===null){popupsEnabled=safeGet(popupSettingKey)!=='0';preferenceChanged();}
    if(event.key===indexKey||event.key===feedKey||event.key===null){
      if(event.key===null)sessionReads.clear();
      ids=loadIndex();notices=null;updateBadge();schedulePopup();
      document.dispatchEvent(new Event('jade-release-change'));
    }else if(event.key.startsWith(readPrefix)){
      if(event.newValue!=='1')sessionReads.delete(event.key.slice(readPrefix.length));
      else if(popupId===event.key.slice(readPrefix.length))document.getElementById('notice-popup').close();
      updateBadge();document.dispatchEvent(new Event('jade-notice-read'));
    }
  });
  updateBadge();
  return {currentVersion,releaseUrl,desktop,entries,isRead,markRead,setPopupsEnabled,setInstallPrompt,
    get popupsEnabled(){return popupsEnabled;},get snapshot(){return snapshot;}};
})();
