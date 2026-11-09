const fs = require('node:fs');
const path = require('node:path');
const {gzipSync} = require('node:zlib');
const read = filename => fs.readFileSync(path.join(__dirname, filename), 'utf8');
const initial = new Set(['db','portraits','trait-data']);

module.exports = function preparePortablePage(html) {
  const payloads = [];
  const shell = html.replace(/<script id="([^"]+)" type="application\/json">([\s\S]*?)<\/script>/g, (tag, id, json) => {
    if (initial.has(id)) return tag;
    let body = json, format = 'text';
    // Already-compressed WebP data moves as a text node; inflating those bytes
    // costs time while saving very little space.
    // The item catalog is also shared by many other screens. Keep it directly
    // readable to avoid a decode on the first navigation on slower CPUs.
    if (id !== 'map-images' && id !== 'item-icons' && id !== 'item-catalog-data') {
      const zipped = gzipSync(json, {level:9}).toString('base64');
      if (zipped.length < Buffer.byteLength(json)*0.72) {body = zipped; format = 'gzip';}
    }
    payloads.push('<script id="embedded-'+id+'" type="application/x-palatlas-data">'+body+'</script>');
    return '<script id="'+id+'" type="application/json" data-embedded-table="'+format+'"></script>';
  });
  const fallback = '/*\n'+read('vendor/pako/LICENSE')+'\n*/\n'+read('vendor/pako/pako_inflate.min.js');
  payloads.push('<script id="embedded-inflate" type="application/x-palatlas-code">'+fallback+'</script>');
  return shell.replace('</head>', () => '<script>'+read('ui/portable-data.js')+'</script>\n</head>')
    .replace('</body>', () => payloads.join('\n')+'\n</body>');
};
