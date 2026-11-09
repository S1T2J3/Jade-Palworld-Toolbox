// The single-file edition keeps optional tables at the end of the same HTML.
// A direct link can ask for a table while the document is still being parsed.
async function readEmbeddedAtlasTable(block) {
  if (document.readyState === 'loading') {
    await new Promise(resolve => document.addEventListener('DOMContentLoaded', resolve, {once:true}));
  }
  const source = document.getElementById('embedded-' + block.id);
  if (!source?.firstChild) throw Error('Missing embedded table ' + block.id);
  if (block.dataset.embeddedTable === 'gzip') {
    const encoded = source.textContent;
    let bytes;
    if (typeof Uint8Array.fromBase64 === 'function') bytes = Uint8Array.fromBase64(encoded);
    else {
      const raw = atob(encoded);
      bytes = new Uint8Array(raw.length);
      for (let i=0; i<raw.length; i++) bytes[i] = raw.charCodeAt(i);
    }
    if (typeof DecompressionStream === 'function') {
      const decoded = new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'));
      block.textContent = await new Response(decoded).text();
    } else {
      if (!window.pako) {
        const script = document.createElement('script');
        script.textContent = document.getElementById('embedded-inflate').textContent;
        document.head.append(script);
        script.remove();
      }
      block.textContent = new TextDecoder().decode(window.pako.inflate(bytes));
    }
  } else {
    block.append(...source.childNodes);
  }
  source.remove();
}
