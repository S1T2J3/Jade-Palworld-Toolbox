function initSkinSettings() {
  const root = document.getElementById('settings-view');
  if (root.dataset.ready) return;
  const options = document.getElementById('sk-options');
  const art = document.getElementById('sk-art');
  const note = document.getElementById('sk-storage-note');
  const themes = [
    ['jade', '玉绿'], ['blue', '海蓝'], ['rose', '玫瑰'],
    ['amber', '琥珀'], ['slate', '岩灰']
  ];
  options.innerHTML = themes.map(([id, name]) =>
    '<label class="sk-card sk-option sk-' + id + '">' +
      '<span class="sk-preview" aria-hidden="true"><i></i><b></b></span>' +
      '<span class="sk-caption"><strong>' + name + '</strong>' +
      '<input type="radio" name="atlas-theme" value="' + id + '"></span></label>'
  ).join('');
  const radios = [...root.querySelectorAll('input[name="atlas-theme"]')];
  function sync() {
    for (const radio of radios) radio.checked = radio.value === document.documentElement.dataset.theme;
    note.hidden = true;
  }
  document.addEventListener('pal-theme-changed', sync);
  root.addEventListener('change', event => {
    if (radios.includes(event.target)) note.hidden = window.PalAtlasTheme.select(event.target.value);
  });
  sync();
  art.src = art.dataset.src;
  art.removeAttribute('data-src');
  root.dataset.ready = 'true';
}
