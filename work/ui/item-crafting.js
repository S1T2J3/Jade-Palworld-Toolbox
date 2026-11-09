// Expand the chosen recipes into a dependency graph before rounding to batches.
// Materials shared by several steps can therefore use the same crafted batch.
function planItemCraft(byId, target, quantity, choices = new Map()) {
  if (!Number.isSafeInteger(quantity) || quantity < 1 || !byId.has(target)) return null;
  const nodes = new Map(), pending = new Set(), order = [];
  function visit(id) {
    if (nodes.has(id)) return;
    pending.add(id);
    const recipes = byId.get(id)?.recipes || [];
    let index = choices.get(id);
    if (!Number.isInteger(index) || index < -1 || index >= recipes.length) {
      index = recipes.findIndex(r => r.materials.every(([material]) => !pending.has(material)));
    }
    const recipe = recipes[index];
    const blocked = new Set();
    nodes.set(id, {id, index, recipe, blocked});
    for (const [material] of recipe?.materials || []) {
      if (pending.has(material)) blocked.add(material);
      else visit(material);
    }
    pending.delete(id);
    order.push(id);
  }
  visit(target);
  const need = new Map([[target,quantity]]), external = new Map(), steps = [], cycles = [];
  const add = (map, id, count) => map.set(id, (map.get(id) || 0) + count);
  for (const id of [...order].reverse()) {
    const {recipe, blocked} = nodes.get(id), required = need.get(id);
    if (!recipe) {
      add(external, id, required);
      continue;
    }
    const batches = Math.ceil(required / recipe.qty);
    const inputs = recipe.materials.map(([material, count]) => [material, count * batches]);
    steps.push({id, required, batches, made:batches * recipe.qty, inputs, recipe});
    for (const [material, count] of inputs) {
      if (blocked.has(material)) {
        cycles.push([id, material, count]);
        add(external, material, count);
      } else add(need, material, count);
    }
  }
  return {steps:steps.reverse(), external, cycles, nodes};
}

function createItemCrafting({byId, detail, itemName, linkItem, escape}) {
  const uses = new Map(), unlocks = new Map(), saved = new Map();
  function index(map, id, product) {
    if (!map.has(id)) map.set(id, new Set());
    map.get(id).add(product);
  }
  for (const item of byId.values()) {
    for (const recipe of item.recipes || []) {
      for (const [id] of recipe.materials) index(uses, id, item.id);
      if (recipe.plan && recipe.plan !== 'None') index(unlocks, recipe.plan, item.id);
    }
  }
  const format = n => n.toLocaleString('zh-CN');
  const textLink = id => linkItem(id, escape(itemName(id)));
  const namedCount = (id, count) => linkItem(id, escape(itemName(id)) + ' <span class="ic-amount">×' + format(count) + '</span>');
  function renderUses(item) {
    function group(map, title) {
      if (!map.has(item.id)) return '';
      const products = [...map.get(item.id)];
      const first = products.slice(0,8).map(textLink).join('');
      const more = products.length > 8 ? '<details class="ic-details-toggle" data-craft-more="' + (map === uses ? 'uses' : 'unlocks') +
        '" data-product="' + escape(item.id) + '"><summary>其余 ' + (products.length - 8) + ' 项</summary><div class="ic-links"></div></details>' : '';
      return '<section class="ic-section"><h3>' + title + ' <small>' + products.length + '</small></h3><div class="ic-links">' + first + '</div>' + more + '</section>';
    }
    return group(uses,'可作为材料制作') + group(unlocks,'可解锁的配方');
  }
  detail.addEventListener('toggle', event => {
    const toggle = event.target;
    if (!toggle.open || !toggle.dataset.craftMore) return;
    const row = toggle.querySelector('.ic-links');
    const map = toggle.dataset.craftMore === 'uses' ? uses : unlocks;
    if (!row.childElementCount) row.innerHTML = [...map.get(toggle.dataset.product)].slice(8).map(textLink).join('');
  }, true);

  function recipeOption(recipe, index) {
    const inputs = recipe.materials.map(([id, count]) => itemName(id) + ' ×' + count).join('、');
    return escape('配方 ' + (index + 1) + ' · ' + inputs + ' → ×' + recipe.qty);
  }
  function selectSource(id, current, root) {
    const recipes = byId.get(id).recipes;
    const selected = value => current === value ? ' selected' : '';
    let options = root ? '' : '<option value="auto"' + selected(undefined) + '>自动展开</option><option value="-1"' + selected(-1) + '>直接取得</option>';
    options += recipes.map((r, i) => '<option value="' + i + '"' + selected(i) + '>' + recipeOption(r,i) + '</option>').join('');
    return '<label class="ic-craft-source">' + (root ? '成品配方' : escape(itemName(id))) + '<select data-craft-source="' +
      escape(id) + '">' + options + '</select></label>';
  }
  const stateFor = id => saved.get(id) || {quantity:1, choices:new Map()};
  function renderPlan(item, state, controlsOpen) {
    const plan = planItemCraft(byId, item.id, state.quantity, state.choices);
    const stocks = [...plan.external].map(([id, count]) => namedCount(id,count)).join('');
    const adjust = [...plan.nodes.values()].filter(node => node.id !== item.id && byId.get(node.id)?.recipes?.length);
    const controls = adjust.length ? '<details class="ic-details-toggle ic-craft-adjust"' + (controlsOpen ? ' open' : '') +
      '><summary>调整中间材料的获取方式</summary><div class="ic-craft-sources">' +
      adjust.map(node => selectSource(node.id,state.choices.get(node.id),false)).join('') + '</div></details>' : '';
    const alerts = plan.cycles.map(([id, material, count]) => '<p class="ic-craft-alert">所选配方形成循环。制作' +
      escape(itemName(id)) + '前需另行取得 ' + namedCount(material,count) + '，或更换配方。</p>').join('');
    const circularMaterials = adjust.filter(node => !node.recipe && state.choices.get(node.id) !== -1);
    const cycleNote = circularMaterials.length ? '<p>' + circularMaterials.map(node => textLink(node.id)).join('、') +
      '的配方会用回上游产物，已按直接取得计算。</p>' : '';
    const plans = [...new Set(plan.steps.map(step => step.recipe.plan).filter(id => id && id !== 'None'))];
    const blueprints = plans.length ? '<h4>所需设计图</h4><div class="ic-links">' + plans.map(textLink).join('') + '</div>' : '';
    const steps = plan.steps.map(step => {
      const rest = step.made - step.required;
      return '<li><div>' + textLink(step.id) + ' <strong>×' + format(step.made) + '</strong><small>' +
        format(step.batches) + ' 批' + (rest ? ' · 余 ' + format(rest) : '') + '</small></div><div class="ic-craft-inputs">' +
        step.inputs.map(([id,count]) => namedCount(id,count)).join('') + '</div></li>';
    }).join('');
    return controls + alerts + cycleNote + '<h4>需备材料</h4><div class="ic-links">' + stocks + '</div>' + blueprints + '<h4>制作顺序</h4>' +
      '<ol class="ic-craft-steps">' + steps + '</ol>';
  }
  function renderPath(item) {
    if (!item.recipes?.length) return '';
    const state = stateFor(item.id);
    return '<section class="ic-section ic-craft-path" data-craft-target="' + escape(item.id) + '"><h3>合成路径</h3>' +
      '<div class="ic-craft-controls"><label>目标数量<input type="number" min="1" max="99999" step="1" inputmode="numeric" data-craft-quantity value="' +
      state.quantity + '"></label>' + (item.recipes.length > 1 ? selectSource(item.id,state.choices.get(item.id) ?? 0,true) : '') +
      '</div><div data-craft-result>' + renderPlan(item,state,false) + '</div></section>';
  }
  function onPlanEdit(event) {
    const input = event.target;
    const source = input.dataset.craftSource;
    if (event.type === 'change' ? !source : !input.hasAttribute('data-craft-quantity')) return;
    const section = input.closest('[data-craft-target]'), id = section.dataset.craftTarget;
    const state = stateFor(id), result = section.querySelector('[data-craft-result]');
    if (source) {
      if (input.value === 'auto') state.choices.delete(source);
      else state.choices.set(source, Number(input.value));
    }
    const quantity = section.querySelector('[data-craft-quantity]');
    if (!quantity.value || !quantity.validity.valid) {
      result.innerHTML = '<p role="status">请输入 1 至 99,999 之间的整数。</p>';
      return;
    }
    state.quantity = quantity.valueAsNumber;
    saved.set(id, state);
    const restoreFocus = result.contains(input);
    result.innerHTML = renderPlan(byId.get(id),state,!!result.querySelector('.ic-craft-adjust[open]'));
    if (restoreFocus) {
      const control = [...result.querySelectorAll('[data-craft-source]')].find(el => el.dataset.craftSource === source);
      control?.focus({preventScroll:true});
    }
  }
  detail.addEventListener('input',onPlanEdit);
  detail.addEventListener('change',onPlanEdit);
  return {renderUses, renderPath};
}

if (typeof module !== 'undefined' && module.exports) module.exports = {planItemCraft};
