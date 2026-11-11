const fs = require('node:fs');
const path = require('node:path');

// Build a small, image-free index once. Search never needs to open every handbook
// table; its links use the same deep routes as the handbook's own references.
function buildSearchIndex({breeding, enemyBook, itemBook, resourceBook, adventureBook, humanLinks} = {}) {
  const read = file => JSON.parse(fs.readFileSync(path.join(__dirname, file), 'utf8'));
  breeding ??= read('data/breeding.json');
  enemyBook ??= read('enemy-data/enemies.json');
  itemBook ??= read('item-data/catalog.json');
  resourceBook ??= read('resource-guides/data.json');
  adventureBook ??= read('adventure/data.json');
  const details = read('pal-details/details.json');
  const partners = read('combat-guide/partners.json');
  const facilities = read('facilities/data.json');
  const expeditions = read('expeditions/data.json');
  const journals = read('journals/data.json');
  const combat = read('combat-guide/combat.json');
  const rows = [], seen = new Set();
  const enc = encodeURIComponent;
  function add(name, kind, href, terms = [], hint = '') {
    if (typeof name !== 'string' || !name.trim() || seen.has(href)) return;
    seen.add(href);
    const aliases = [...new Set(terms.flat().filter(t => typeof t === 'string' && t.trim() && t !== name))];
    rows.push({name, kind, href, ...(aliases.length ? {aliases} : {}), ...(hint ? {hint} : {})});
  }
  for (const p of breeding.species) {
    add(p.zh, '帕鲁', '#paldex/' + enc(p.key), [p.key, p.en, p.dex, '#' + p.dex, ...(p.aliases || [])], '#' + p.dex + ' · ' + p.en);
  }
  for (const [id, m] of Object.entries(details.moves)) {
    add(m.zh, '主动技能', '#details/active/move/' + enc(id), [id, m.en], (combat.elements[m.type] || m.type) + '属性 · 威力 ' + m.power);
  }
  for (const [id, p] of Object.entries(details.passives)) {
    add(p.zh, '被动词条', '#details/passive/trait/' + enc(id), [id, p.en], p.effect || '');
    if (p.surgery) add(p.zh, '手术台', '#details/surgery/trait/' + enc(id), [id, p.en, p.surgery.name], '可通过手术添加的词条');
  }
  const pals = new Map(breeding.species.map(p => [p.key, p]));
  for (const [id, skill] of Object.entries(partners)) {
    const pal = pals.get(id);
    if (pal) add(skill.name, '伙伴技能', '#details/partner/' + enc(id), [id, pal.zh, pal.en], pal.zh + ' · #' + pal.dex);
  }
  for (const i of itemBook.items) {
    add(i.name, '物品', '#items/' + enc(i.id), [i.id, i.en, i.slug, ...(i.aliases || [])], i.category || '');
  }
  for (const f of facilities.facilities) {
    add(f.name, '设施', '#work/facilities/' + enc(f.id), [f.id, f.en, ...(f.aliases || [])], f.group || f.category || '');
  }
  for (const e of enemyBook.enemies) {
    add(e.name, '敌人', '#enemies/' + enc(e.id), [e.id, e.en, ...(e.aliases || [])], [e.group, e.variant].filter(Boolean).join(' · '));
  }
  for (const p of enemyBook.people || []) {
    const merchant = p.category === 'merchants';
    add(p.name, merchant ? '商人' : '中立单位', humanLinks?.unitRoutes?.[p.id] || '#' + (merchant ? 'merchants' : 'neutrals') + '/' + enc(p.id),
      [p.id, p.en, ...(p.aliases || []), ...(p.sourceIds || [])], p.group || '');
  }
  for (const e of expeditions.stages) {
    add(e.name, '远征', '#work/expeditions/' + enc(e.id), [e.id, e.region], e.region + (e.hard ? ' · 高难度' : ''));
  }
  for (const j of journals.catalog) {
    add(j.title, '手记', '#guide/journals/' + enc(j.id), [j.id, j.group], j.group || '');
  }
  for (const q of adventureBook.quests) {
    add(q.name, '任务', '#guide/quests/' + enc(q.id), [q.id, q.code], q.group || '');
  }
  for (const s of combat.status) {
    add(s.name, '异常状态', '#guide/status/' + enc(s.id), [s.id], s.group || '');
  }
  for (const g of resourceBook.gathering) {
    add(g.name, '野外采集', '#guide/gathering/' + enc(g.id), [g.id], g.group || '');
  }
  for (const p of resourceBook.production) {
    add(p.name, '据点生产', '#work/production/' + enc(p.id), [p.id, p.itemName, ...(p.aliases || [])], p.itemName || '');
  }
  for (const c of adventureBook.chapters) {
    add(c.name, '探索奖励', '#guide/loot/' + enc(c.id), [c.id]);
    for (const p of c.pools) {
      add(p.name, '探索奖励', '#guide/loot/' + enc(c.id) + '/' + enc(p.id), [p.id, c.name], c.name);
    }
  }
  return rows;
}

module.exports = buildSearchIndex;
module.exports.buildSearchIndex = buildSearchIndex;
