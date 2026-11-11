//! Native algorithms for Jade. No web engine or JavaScript interpreter is used.
pub mod breeding;
pub mod calculators;
pub mod crafting;
pub use breeding::{trait_solve, plan_request, warehouse_solve, warehouse_route, offspring_solve, recipe_pairs};
pub use calculators::{calculate_pal_stats, estimate_capture, calculate_work_efficiency, relic_product_odds, simulate_relic};
pub use crafting::plan_item_craft;
