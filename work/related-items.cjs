// Build reverse references from the same records that supply the forward links.
module.exports = function relatedItems(details, partners, combat) {
  const links = {};
  function add(item, path, name) {
    (links[item] ||= []).push([path, name]);
  }
  for (const [id, move] of Object.entries(details.moves)) {
    if (move.fruit) add('SkillCard_' + id, '#details/active/move/' + encodeURIComponent(id), '主动技能 · ' + move.zh);
  }
  for (const [id, trait] of Object.entries(details.passives)) {
    if (!trait.surgery?.item) continue;
    const key = encodeURIComponent(id);
    add(trait.surgery.item, '#details/passive/trait/' + key, '被动技能 · ' + trait.zh);
    add(trait.surgery.item, '#details/surgery/trait/' + key, '手术条件 · ' + trait.zh);
  }
  for (const [pal, record] of Object.entries(partners)) {
    for (const [item] of record.gear || []) {
      add(item, '#details/partner/' + encodeURIComponent(pal), '伙伴技能 · ' + record.name);
    }
  }
  for (const state of combat?.status || []) {
    if (state.medicine) add(state.medicine[0], '#guide/status/' + encodeURIComponent(state.id), '治疗 · ' + state.name);
  }
  return links;
};
