# jade-core

Rust-only application algorithms. Runtime does not call JavaScript, Node, C#,
Go, a browser, or a JavaScript interpreter. The development-only Node tool
records v1.0.17 reference outputs to check migration parity.

## JSON interface

- `recipe_pairs(db)`: returns recipe rows from full or compact breeding data.
- `trait_solve(request)` / `plan_request(request)`: request fields retain the
  original UI schema: `pairs`, `speciesCount`, `stock`, `target`, `traits`,
  `allowRandom`, `randomEligible`, `surgeryOptions`, `requireHatch`, `preferHatch`.
  Stock rows are `{id, species: <numeric species index>, sex: M|F|unknown|any,
  traits: [trait keys]}`. `unknown` is one physical individual with one fixed,
  unknown sex; `any` retains legacy species-pool semantics. Surgery entries are
  `{key,mode,cost,...}`. `plan_request` is the UI entry point and preserves
  deterministic-first, prefer-new-hatch behavior. Preserve every returned
  `conditionalSex`, `conditional`, `minimumUnconfirmed`, `hatchUnavailable`
  warning in the UI; these are meaningful limits, not successes to hide.
- `offspring_solve(request)`: `{pairs,speciesCount,stock}` → `{items,unresolved}`.
  Each item retains `depth`, `steps`, `conditional`, `minimumCertain`.
- `warehouse_solve(pairs,count,owned,max_steps)`: structural plan. Unreachable
  depth is JSON `null` (the serialized equivalent of JavaScript Infinity).
- `warehouse_route(plan,target,include_owned)`: ordered recipe rows.
- `calculate_pal_stats(profile,config,passive)`: profile `{growth:[hp,attack,defense],
  trust:[hp,attack,defense]}`; config `{level,stars,trust,awaken,
  iv:{hp,attack,defense},souls:{hp,attack,defense,work}}`; passive is the summed
  percent modifiers `{hp,attack,defense,work}`. Output `{panel,expedition,power,factor}`.
- `estimate_capture(config)`: `{power,level,hp,coefficient,world,status,blocked}`
  → `{reticle,chance}`; probabilities in [0,1]. UI still supplies sphere/module,
  effigy, partner, critical and encounter coefficients and blocks uncatchable bosses.
- `calculate_work_efficiency(data,job,level,keys)`: original work-efficiency data;
  errors for >4 unique traits or invalid job/level. Output
  `{rank,base,work,ranch,movement,amount}`.
- `plan_item_craft(items,target,quantity,choices)`: `items` may be a keyed object
  or item array, `choices` is `{itemId: recipeIndex}` (-1=direct acquisition).
  Output `steps`, ordered `external` pairs, `cycles`, ordered `nodes` (including
  selected recipe and blocked cycle edges).
- `relic_product_odds(tier,pools)`: ordered product array with
  `{id,probability,min,max,slots}`. Pools can be arrays or keyed objects.
- `simulate_relic(tier,pools,rng)`: ordered `[itemId,count]` pairs. Caller supplies
  independent random draws in [0,1); this is a simulator, not a game prediction.

Inputs come from validated UI state and shipped data. Unknown save identities,
removed inventory and per-world state must be filtered by the caller before
planning, exactly as with the old application.

## Validation

`cargo test -p jade-core` checks the native output against deterministic reference
fixtures, including complete witness paths, ordering and warnings. Float-only
calculator output uses a relative 1e-9 comparison; breeding output is exact.

`cargo run --release -p jade-core --example measure -- cases.json` measures five
in-process samples after two warm-ups. It excludes file read/JSON parse and GUI
paint; do not label these measurements full application latency.
