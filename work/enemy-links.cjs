// Keep reverse links small enough to ship with the tables that use them.
// Enemy portraits and detailed loot rows stay in the enemy catalog.
module.exports = function connectEnemies(book, items, pals, points) {
  const itemNames = new Map(items.map(item => [item.id, item.name]));
  const palKeys = new Set(pals.map(pal => pal.key));
  const pointIds = new Set(points.map(point => point.id));
  const index = {itemNames:{}, enemyDrops:{}, enemyNames:{}, enemyFirstRewards:{}, enemySummons:{}, byPoint:{}, byPal:{}};
  const labels = new Map();
  for (const enemy of book.enemies) {
    const name = enemy.name + (enemy.variant && enemy.variant !== enemy.name ? '（' + enemy.variant + '）' : '');
    if (!labels.has(name)) labels.set(name, []);
    labels.get(name).push(enemy);
  }
  const seen = new Set();
  function add(target, key, enemy, label) {
    const links = target[key] ||= [];
    if (!links.some(([id]) => id === enemy.id)) links.push([enemy.id, label]);
  }
  function addItem(target, id, enemy, label) {
    if (!itemNames.has(id)) throw Error('Unknown enemy item ' + id + ' on ' + enemy.id);
    index.itemNames[id] = itemNames.get(id);
    add(target, id, enemy, label);
  }
  for (const enemy of book.enemies) {
    if (!enemy.id || !enemy.name || seen.has(enemy.id)) throw Error('Invalid enemy ID ' + enemy.id);
    seen.add(enemy.id);
    const name = enemy.name + (enemy.variant && enemy.variant !== enemy.name ? '（' + enemy.variant + '）' : '');
    const same = labels.get(name);
    let label = name;
    if (same.length > 1) {
      const distinctWeapon = enemy.weapon && same.filter(other=>other.weapon===enemy.weapon).length===1;
      enemy.detailLabel = distinctWeapon ? enemy.weapon : enemy.id;
      label += ' · ' + enemy.detailLabel;
    }
    add(index.enemyNames, enemy.name, enemy, label);
    for (const tier of [enemy, ...(enemy.dropTiers || [])]) {
      for (const [id] of tier.drops || []) addItem(index.enemyDrops, id, enemy, label);
    }
    if (enemy.firstReward) addItem(index.enemyFirstRewards, enemy.firstReward, enemy, label);
    if (enemy.summonItem) addItem(index.enemySummons, enemy.summonItem, enemy, label);
    if (enemy.palKey) {
      if (!palKeys.has(enemy.palKey)) throw Error('Unknown enemy Pal ' + enemy.palKey);
      add(index.byPal, enemy.palKey, enemy, label);
    }
    for (const point of enemy.points || []) {
      if (!pointIds.has(point.id)) throw Error('Unknown enemy point ' + point.id);
      add(index.byPoint, point.id, enemy, label);
    }
  }
  return index;
};
