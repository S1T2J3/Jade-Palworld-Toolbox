// The update controls are loaded on the first visit or after a download completes.
function createUpdateInstaller(){
  const byId=id=>document.getElementById(id),dialog=byId('install-update');
  const now=byId('install-update-now'),later=byId('install-update-later'),close=byId('install-update-close');
  const feedback=byId('install-update-status');
  let displayed='',requested='',requestId='',message='',timer;
  function render(){
    now.disabled=later.disabled=close.disabled=Boolean(requestId);
    feedback.textContent=displayed===requested?message:'';
    feedback.hidden=!feedback.textContent;
    document.dispatchEvent(new Event('jade-install-change'));
  }
  function restart(version){
    if(!version||requestId)return;
    requested=version;
    requestId='restart-'+(globalThis.crypto?.randomUUID?.()||Date.now().toString(36));
    message='正在准备重启并安装 v'+version+'…';
    timer=setTimeout(()=>{
      requestId='';message='工具箱仍在运行。可以重试，或选择下次启动时安装。';render();
    },60000);
    render();
    document.dispatchEvent(new CustomEvent('pal-release-request',{bubbles:true,
      detail:JSON.stringify({action:'restart',version,requestId})}));
  }
  document.addEventListener('jade-release-change',()=>{
    const result=jadeReleases.snapshot;
    if(!requestId||result.requestId!==requestId)return;
    if(result.state==='restarting')message='正在关闭工具箱，随后将打开新版…';
    else{
      clearTimeout(timer);requestId='';
      message=result.message||'未能重启工具箱。请重试，或选择下次启动时安装。';
    }
    render();
  });
  later.addEventListener('click',()=>dialog.close());
  close.addEventListener('click',()=>dialog.close());
  dialog.addEventListener('cancel',event=>{if(requestId)event.preventDefault();});
  now.addEventListener('click',()=>restart(displayed));
  jadeReleases.setInstallPrompt(version=>{
    if(dialog.open)return false;
    displayed=version;
    byId('install-update-description').textContent='v'+version+' 已下载并通过校验。你可以立即安装，或继续使用，在下次打开工具箱时自动安装。';
    render();dialog.showModal();return true;
  });
  return {restart,get busy(){return Boolean(requestId);},status:version=>requested===version?message:''};
}
const jadeInstaller=jadeReleases.desktop?createUpdateInstaller():null;
let releaseCenterRefresh;
function showReleaseCenter(){
  if(!releaseCenterRefresh)releaseCenterRefresh=initReleaseCenter();
  releaseCenterRefresh();
}
function initReleaseCenter(){
  const releases=jadeReleases, byId=id=>document.getElementById(id);
  const list=byId('notice-list'), all=byId('notice-read-all');
  const popupSwitch=byId('notice-popup-enabled');
  const version=byId('release-version'), status=byId('release-status'), date=byId('release-date');
  const auto=byId('release-auto'), check=byId('release-check'), download=byId('release-download'),restart=byId('release-restart');
  const token=byId('release-token'), machine=byId('release-machine'), clear=byId('release-clear');
  const link=byId('release-link'),access=byId('release-access'),notes=byId('release-notes');
  let rows=null, controls=new Map(), pending=null, lastFailure='', localError='', lastState='';
  const dates=new Intl.DateTimeFormat('zh-CN',{year:'numeric',month:'long',day:'numeric'});
  const times=new Intl.DateTimeFormat('zh-CN',{dateStyle:'medium',timeStyle:'short'});

  function refreshPopupSetting(){popupSwitch.checked=releases.popupsEnabled;}
  refreshPopupSetting();
  popupSwitch.addEventListener('change',()=>releases.setPopupsEnabled(popupSwitch.checked));
  document.addEventListener('jade-popup-setting',refreshPopupSetting);

  function refreshRead(){
    let unread=0;
    for(const [id,{badge}] of controls){
      badge.hidden=releases.isRead(id);
      if(!badge.hidden)unread++;
    }
    all.hidden=unread===0;
  }
  function refreshNotices(){
    const entries=releases.entries();
    if(entries===rows)return;
    const old=controls, fragment=document.createDocumentFragment();
    controls=new Map();rows=entries;
    for(const row of entries){
      const fold=document.createElement('details'),summary=document.createElement('summary');
      const title=document.createElement('span'),badge=document.createElement('span'),body=document.createElement('p');
      title.className='notice-title';title.textContent=row.title;
      badge.className='notice-unread';badge.textContent='未读';
      body.className='notice-body';body.textContent=row.body;
      summary.append(title,badge);fold.append(summary);
      const timestamp=Date.parse(row.publishedAt);
      if(Number.isFinite(timestamp)){
        const time=document.createElement('time');time.dateTime=row.publishedAt;time.textContent=dates.format(timestamp);fold.append(time);
      }
      fold.append(body);fragment.append(fold);
      fold.open=old.get(row.id)?.fold.open||false;
      fold.addEventListener('toggle',()=>{
        if(fold.open&&fold.isConnected&&!releases.isRead(row.id))releases.markRead([row.id]);
      });
      controls.set(row.id,{fold,summary,badge});
    }
    const focused=[...old].find(([,entry])=>entry.summary===document.activeElement)?.[0];
    list.replaceChildren(fragment);refreshRead();
    if(focused)controls.get(focused)?.summary.focus({preventScroll:true});
  }
  all.addEventListener('click',()=>{
    releases.markRead([...controls.keys()]);
    controls.values().next().value?.summary.focus({preventScroll:true});
  });
  document.addEventListener('jade-notice-read',refreshRead);
  if(releases.releaseUrl){link.href=releases.releaseUrl;link.hidden=false;}
  version.textContent='当前版本 v'+releases.currentVersion;
  byId('release-desktop').hidden=!releases.desktop;
  if(!releases.desktop){
    status.textContent='当前使用离线网页版。可从 GitHub 发布页下载新版或 Windows 桌面版。';
    document.addEventListener('jade-release-change',refreshNotices);
    return refreshNotices;
  }

  const actionLabel={
    status:'正在读取更新设置…',check:'正在检查更新与公告…',download:'正在下载并校验更新包…',
    setAuto:'正在保存自动更新设置…',setAccess:'请在 Windows 凭据窗口中完成绑定。',useMachine:'正在切换到本机登录…',clearAccess:'正在清除授权…'
  };
  function isNewer(candidate,baseline){
    if(!candidate||!baseline)return false;
    const target=candidate.split('.').map(Number),previous=baseline.split('.').map(Number);
    for(let i=0;i<4;i++){
      const difference=(target[i]||0)-(previous[i]||0);
      if(difference)return difference>0;
    }
    return false;
  }
  function render(){
    refreshNotices();
    const data=releases.snapshot,latest=data.latestVersion||'',ready=data.readyVersion||'';
    const state=data.state,enabled=!pending&&!jadeInstaller.busy;
    version.textContent='当前版本 v'+(data.currentVersion||releases.currentVersion);
    const stateLabel={
      idle:'可以检查工具箱更新和最新公告。',current:'已是最新版本。',
      available:latest?'可更新至 v'+latest+'。':'发现新版本。',ready:'',restarting:'正在重启工具箱…',
      unauthorized:'当前账号无法读取此私有仓库。请检查 GitHub 账号的仓库权限，或绑定访问令牌。',
      unconfigured:'此版本尚未配置更新来源。',busy:'后台正在处理更新，稍后可再次检查。',
      error:'检查更新失败，请稍后重试。'
    };
    const checkMessage=['current','available','ready'].includes(state)
      ?[stateLabel[state],data.message].filter(Boolean).join(' '):data.message||stateLabel[state]||'';
    let message=pending?actionLabel[pending.action]:localError||checkMessage;
    if(ready&&!pending){
      const kind={delta:'（增量下载）',full:'（完整下载）'}[data.downloadKind]||'';
      const readyText='v'+ready+' 已就绪'+kind+'，可立即重启或在下次启动时安装。';
      message=[readyText,message].filter(Boolean).join(' ');
    }
    if(jadeInstaller.status(ready))message=jadeInstaller.status(ready);
    status.textContent=message;
    const checked=Date.parse(data.checkedAt);
    date.hidden=!Number.isFinite(checked);
    date.textContent=date.hidden?'':'上次检查 '+times.format(checked);
    notes.hidden=!data.releaseNotes;
    byId('release-notes-body').textContent=typeof data.releaseNotes==='string'?data.releaseNotes.slice(0,23000):'';
    auto.checked=pending?.action==='setAuto'?pending.enabled:data.auto!==false;
    check.textContent=lastFailure==='check'?'重试检查':'检查更新';
    const canDownload=(state==='available'||state==='error'||lastFailure==='download')&&isNewer(latest,ready||data.currentVersion);
    download.hidden=!canDownload;
    download.textContent=lastFailure==='download'?'重试下载':'下载更新';
    restart.hidden=!ready;
    for(const control of [auto,check,download,restart,token,machine,clear])control.disabled=!enabled;
    const isPublic=releases.publicChannel;
    access.hidden=isPublic&&!data.hasStoredCredential;
    byId('release-access-title').textContent=isPublic?'旧版授权':'仓库访问';
    byId('release-token-help').hidden=isPublic;
    token.hidden=isPublic;
    machine.hidden=isPublic||data.accessMode==='machine';
    clear.hidden=isPublic?!data.hasStoredCredential:data.accessMode==='off';
    clear.textContent=isPublic?'清除旧令牌':'清除授权';
    token.textContent=data.accessMode==='stored'?'更换访问令牌':'绑定访问令牌';
    byId('release-access-info').textContent=({
      public:'官方公告和更新使用公开渠道。你可以清除旧版保存的令牌。',stored:'当前使用已保存的访问令牌。',off:'已清除授权，本机 GitHub 登录访问已停用。',machine:'当前使用本机 GitHub 登录。'
    })[data.accessMode]||'正在读取仓库访问方式。';
    if(!isPublic&&state==='unauthorized'&&lastState!==state)access.open=true;
    lastState=state;
  }
  function request(action,enabled){
    if(pending)return;
    const requestId='release-'+Date.now().toString(36)+'-'+Math.random().toString(36).slice(2,10);
    const payload={action,requestId};
    if(action==='setAuto')payload.enabled=enabled;
    pending={action,requestId,enabled};
    // Long downloads and the native credential prompt may outlive a network check.
    const timeout={status:10000,download:15*60000,setAccess:10*60000}[action]||60000;
    pending.timer=setTimeout(()=>{
      pending=null;lastFailure=action;localError='尚未收到桌面程序的响应，可稍后重试。';
      render();
    },timeout);
    localError='';render();
    document.dispatchEvent(new CustomEvent('pal-release-request',{bubbles:true,detail:JSON.stringify(payload)}));
  }
  document.addEventListener('jade-release-change',()=>{
    const result=releases.snapshot;
    if(pending&&pending.requestId===result.requestId){
      const action=pending.action;
      clearTimeout(pending.timer);pending=null;
      if(['error','unauthorized'].includes(result.state))lastFailure=action;
      else if(action!=='status')lastFailure='';
      localError='';
    }
    render();
  });
  document.addEventListener('jade-install-change',render);
  restart.addEventListener('click',()=>jadeInstaller.restart(releases.snapshot.readyVersion));
  check.addEventListener('click',()=>request('check'));
  download.addEventListener('click',()=>request('download'));
  auto.addEventListener('change',()=>request('setAuto',auto.checked));
  token.addEventListener('click',()=>request('setAccess'));
  machine.addEventListener('click',()=>request('useMachine'));
  clear.addEventListener('click',()=>request('clearAccess'));
  return ()=>{render();request('status');};
}
