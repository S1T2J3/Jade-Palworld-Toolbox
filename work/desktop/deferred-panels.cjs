// Keep section anchors and data-table IDs available to the router. Mount
// the contents of each secondary panel with its own JS on the first visit.
const panelModules = {
  'spawns-view':'spawns','pal-details-view':'details','paldex-view':'paldex',
  'items-view':'items','enemies-view':'enemies','adventure-view':'adventure',
  'gathering-view':'resource-guides','production-view':'resource-guides',
  'facilities-view':'facilities','work-efficiency-view':'work-efficiency',
  'ranch-view':'ranch','relic-view':'relic','combat-guide-view':'guide',
  'journals-view':'journals','partner-skills-view':'partner','expeditions-view':'expeditions',
  'settings-view':'settings', 'updates-view':'updates'
};

function splitPanels(page, append) {
  for (const [id,feature] of Object.entries(panelModules)) {
    const start=page.indexOf('<section id="'+id+'"');
    if(start<0) throw new Error('Missing panel '+id);
    const content=page.indexOf('>',start)+1;
    const sections=/<\/?section\b[^>]*>/gi;
    sections.lastIndex=content;
    let depth=1, tag;
    while(depth && (tag=sections.exec(page))) depth+=tag[0].startsWith('</')?-1:1;
    if(depth) throw new Error('Unclosed panel '+id);
    const tables=[];
    const markup=page.slice(content,tag.index).replace(/<script\b[^>]*>[\s\S]*?<\/script>/g,node=>{
      if(!node.includes('type="application/json"')) throw new Error('Unexpected executable script in '+id);
      tables.push(node);return '';
    });
    append(feature,'mountAtlasPanel('+JSON.stringify(id)+','+JSON.stringify(markup)+');');
    page=page.slice(0,content)+'</section>'+tables.join('\n')+page.slice(sections.lastIndex);
  }
  return page;
}
module.exports={splitPanels};
