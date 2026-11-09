const WarehouseSolver = (() => {
  function solve(pairs, speciesCount, ownedIndices, maxSteps = Infinity) {
    const depth = Array(speciesCount).fill(Infinity);
    const recipe = Array(speciesCount).fill(null);
    const bredDepth = Array(speciesCount).fill(Infinity);
    const bredRecipe = Array(speciesCount).fill(null);
    const plan = {depth, recipe, bredDepth, bredRecipe};
    for (const index of ownedIndices) {
      if (Number.isInteger(index) && index >= 0 && index < speciesCount) depth[index] = 0;
    }
    if (!depth.includes(0)) return plan;

    // Advance one generation at a time. A new child cannot act as a parent
    // in the same generation, even when its recipe appears earlier in the table.
    for (let step = 1; step <= maxSteps; step++) {
      let added = false;
      for (const row of pairs) {
        const [a, b, child] = row;
        if (bredDepth[child] < step || Math.max(depth[a], depth[b]) !== step - 1) continue;

        const previous = bredRecipe[child];
        // Within the earliest generation, prefer a shallower second branch.
        // Source-table order breaks remaining ties in a reproducible way.
        if (previous && Math.min(depth[a], depth[b]) >= Math.min(depth[previous[0]], depth[previous[1]])) continue;
        bredDepth[child] = step;
        bredRecipe[child] = row;
        // Existing stock stays immediately available, even when the earliest
        // way to hatch another of that species uses newly raised parents.
        if (depth[child] !== 0) {
          depth[child] = step;
          recipe[child] = row;
          added = true;
        }
      }
      if (!added) break;
    }
    return plan;
  }

  function route(plan, targetIndex, includeOwned = false) {
    const finalRow = (includeOwned ? plan.bredRecipe : plan.recipe)[targetIndex];
    if (!finalRow) return [];
    const ordered = [];
    const seen = new Set();
    function visit(index) {
      const row = plan.recipe[index];
      if (!row || seen.has(index)) return;
      seen.add(index);
      visit(row[0]);
      visit(row[1]);
      ordered.push(row);
    }
    visit(finalRow[0]);
    visit(finalRow[1]);
    ordered.sort((first, second) => plan.depth[first[2]] - plan.depth[second[2]]);
    ordered.push(finalRow);
    return ordered;
  }

  return {solve, route};
})();

if (typeof window !== 'undefined') window.WarehouseSolver = WarehouseSolver;
if (typeof module !== 'undefined' && module.exports) module.exports = WarehouseSolver;
