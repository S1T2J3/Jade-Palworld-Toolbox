// Resolve item and Pal shop lists once at build time. Maps and the item catalog
// only need reverse links, so browsing the shop never loads the entire item book.
module.exports = function connectPeople(book, itemBook, species, mapPoints) {
  const items = new Map(itemBook.items.map(i => [i.id,i.name]));
  const pals = new Set(species.map(p => p.key));
  const extraPoints = book.extraPoints || [];
  const points = new Set([...mapPoints,...extraPoints].map(p => p.id));
  const result = {extraPoints,itemNames:{},unitsByShop:{},byPoint:{},dropsByItem:{},namesByItem:{},palSellers:{},requestRewards:{},unitRoutes:{}};
  function add(target,key,route,label) {
    const rows = target[key] ||= [];
    if (!rows.some(([r]) => r === route)) rows.push([route,label]);
  }
  function item(id) {
    if (!items.has(id)) throw Error('Unknown unit item ' + id);
    result.itemNames[id] = items.get(id);
  }
  book.shopLists = Object.fromEntries(Object.entries(itemBook.shops).map(([id,s]) =>
    [id,{name:s.name,note:s.note,rows:[]}]));
  for (const product of itemBook.items) for (const shop of product.sellers || []) {
    const list = book.shopLists[shop.shop];
    if (!list) throw Error('Unknown item shop ' + shop.shop);
    item(product.id);item(shop.currency);
    list.rows.push([product.id,shop.qty,shop.cost,shop.currency,Boolean(shop.once)]);
  }
  const names = new Map();
  const display = person => person.name + (person.variant && person.variant !== person.group && person.variant !== person.name ? '（'+person.variant+'）' : '');
  for (const person of book.people || []) {
    const label = display(person);
    names.set(label,(names.get(label)||0)+1);
  }
  for (const enemy of book.enemies) result.unitRoutes[enemy.id] = '#enemies/'+encodeURIComponent(enemy.id);
  for (const person of book.people || []) {
    const route = '#'+person.category+'/'+encodeURIComponent(person.id);
    let label = display(person);
    if (names.get(label)>1) {
      const p = person.points?.[0];
      person.detailLabel = p ? p.x+'，'+p.y : person.id;
      label += ' · '+person.detailLabel;
    }
    if (result.unitRoutes[person.id]) throw Error('Conflicting unit ID ' + person.id);
    result.unitRoutes[person.id] = route;
    for (const id of person.sourceIds || []) result.unitRoutes[id] = route;
    for (const id of person.shops || []) {
      if(!book.shopLists[id]?.rows.length) throw Error('Empty merchant shop ' + id);
      add(result.unitsByShop,id,route,label);
    }
    for (const pal of person.palStock?.pals || []) {
      if(!pals.has(pal)) throw Error('Unknown merchant Pal ' + pal);
      add(result.palSellers,pal,route,label);
    }
    for (const request of person.requests || []) {
      if(request.item)item(request.item);
      for(const [id] of request.rewards) {
        item(id);add(result.requestRewards,id,route,label);
      }
    }
    for (const p of person.points || []) {
      if(!points.has(p.id)) throw Error('Unknown NPC point ' + p.id);
      add(result.byPoint,p.id,route,label);
    }
    for (const tier of [person,...person.dropTiers||[]]) for (const [id] of tier.drops || []) {
      item(id);add(result.dropsByItem,id,route,label);
      const named = result.namesByItem[id] ||= {};
      for (const name of [person.name,...person.aliases||[]]) add(named,name,route,label);
    }
  }
  return result;
};
