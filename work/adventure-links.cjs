// Reverse lookups travel with the item table. Opening an item doesn't need to
// parse the full quest text, reward pools, maps or production notes.
module.exports = function adventureLinks(adventure, resources, items) {
  const known = new Set(items.map(item => item.id));
  const links = {};
  function add(id, group, route, label) {
    if (!known.has(id)) throw Error('Unknown guide item ' + id + ' in ' + route);
    const groups = links[id] ||= {};
    const entries = groups[group] ||= [];
    if (!entries.some(([url]) => url === route)) entries.push([route, label]);
  }
  for (const row of resources.gathering) add(row.id,'野外采集','#guide/gathering/'+encodeURIComponent(row.id),row.name);
  for (const row of resources.production) add(row.item,'据点生产','#work/production/'+encodeURIComponent(row.id),row.name);
  for (const quest of adventure.quests) {
    for (const reward of quest.rewards) {
      if (reward.item) add(reward.item,'任务奖励','#guide/quests/'+encodeURIComponent(quest.id),quest.name);
    }
  }
  for (const chapter of adventure.chapters) {
    const route = '#guide/loot/' + encodeURIComponent(chapter.id) + '/item/';
    for (const id of new Set(chapter.pools.flatMap(pool=>pool.rows.map(row=>row[0])))) {
      add(id,'探索奖励表',route + encodeURIComponent(id),chapter.name);
    }
  }
  return links;
};
