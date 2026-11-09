// Keep one full portrait inline for the selected specimen. The default
// breeding rows use compact local previews, while detail views retain the
// original assets independently of which species the user opened first.
function splitDesktopImages(html,splitImages,breeding) {
  const tag='<script id="portraits" type="application/json">';
  const start=html.indexOf(tag);
  const end=html.indexOf('</script>',start);
  if(start<0 || end<start) throw Error('Missing portrait catalog');
  const portraits=JSON.parse(html.slice(start+tag.length,end));
  for(const [name,uri] of Object.entries(portraits)) {
    if(name!==breeding.species[0].en) portraits[name]=splitImages(uri);
  }
  const preview='<script id="lookup-thumbnails" type="application/x-palatlas-portraits">'+JSON.stringify(require('./lookup-thumbnails.json'))+'</script>';
  return splitImages(html.slice(0,start))+preview+tag+JSON.stringify(portraits)+splitImages(html.slice(end));
}
module.exports={splitDesktopImages};
