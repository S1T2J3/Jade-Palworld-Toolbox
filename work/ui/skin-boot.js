(() => {
  const key = 'pal-atlas.theme.v1';
  const ids = ['moon', 'jade', 'blue', 'rose', 'amber', 'slate'];
  let storage;
  try { storage = window.localStorage; } catch {}

  function syncBrowserColor() {
    const tag=document.querySelector('meta[name="theme-color"]');
    const color=getComputedStyle(document.documentElement).getPropertyValue('--skin-rail').trim();
    if(tag&&color)tag.content=color;
  }
  document.addEventListener('DOMContentLoaded',syncBrowserColor,{once:true});
  function saved() {
    try { return storage?.getItem(key); } catch { return null; }
  }
  function apply(id) {
    const theme = ids.includes(id) ? id : 'moon';
    if (document.documentElement.dataset.theme === theme) return;
    document.documentElement.dataset.theme = theme;
    if(document.readyState!=='loading')syncBrowserColor();
    document.dispatchEvent(new Event('pal-theme-changed'));
  }
  window.PalAtlasTheme = {
    select(id) {
      if (!ids.includes(id)) return false;
      let persisted = false;
      try {
        if (storage) { storage.setItem(key, id); persisted = true; }
      } catch {}
      apply(id);
      return persisted;
    }
  };
  apply(saved());
  window.addEventListener('storage', event => {
    if (event.storageArea === storage && (event.key === key || event.key === null)) apply(saved());
  });
})();
