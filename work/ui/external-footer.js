function bindExternalFooter(loader) {
  const footer=document.getElementById('external-directory');
  if(!footer)return;
  const status=footer.querySelector('[data-external-status]');
  const retry=footer.querySelector('[data-external-retry]');
  let pending=false,ready=false;
  async function open() {
    if(!footer.open||pending||ready)return;
    pending=true;
    status.hidden=false;status.textContent='正在读取本地链接目录…';retry.hidden=true;
    try {
      const ids=['code-external-links','item-catalog-data','facilities-data','journals-data','ranch-data','capture-data'];
      await Promise.all(ids.map(id=>loader.load(id)));
      const read=id=>JSON.parse(document.getElementById(id).textContent);
      initExternalDirectory(footer,buildExternalDirectory({
        catalog:read('item-catalog-data'),facilities:read('facilities-data').facilities,
        journals:read('journals-data').catalog,ranch:read('ranch-data').pals,
        capture:read('capture-data'),species:read('db').species,releaseUrl:jadeReleases.releaseUrl
      }));
      ready=true;status.hidden=true;
    }catch{
      status.textContent='未能读取本地链接目录。';retry.hidden=false;
    }finally{pending=false;}
  }
  footer.addEventListener('toggle',open);
  retry.addEventListener('click',open);
}
