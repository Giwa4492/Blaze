use std::sync::Arc;

use penumbra_asset::asset;
use penumbra_num::fixpoint::U128x128;

#[derive(Debug, Clone)]
pub struct RoutingParams {
    pub price_limit: Option<U128x128>,
    pub fixed_candidates: Arc<Vec<asset::Id>>,
    pub max_hops: usize,
}

impl Default for RoutingParams {
    fn default() -> Self {
        Self {
            price_limit: None,
            fixed_candidates: Arc::new(vec![
                asset::Cache::with_known_assets()
                    .get_unit("test_usd")
                    .expect("hardcoded \"test_usd\" denom should be known")
                    .id(),
                asset::Cache::with_known_assets()
                    .get_unit("penumbra")
                    .expect("hardcoded \"penumbra\" denom should be known")
                    .id(),
                asset::Cache::with_known_assets()
                    .get_unit("gm")
                    .expect("hardcoded \"gm\" denom should be known")
                    .id(),
                asset::Cache::with_known_assets()
                    .get_unit("gn")
                    .expect("hardcoded \"gn\" denom should be known")
                    .id(),
                asset::Cache::with_known_assets()
                    .get_unit("test_atom")
                    .expect("hardcoded \"test_atom\" denom should be known")
                    .id(),
                asset::Cache::with_known_assets()
                    .get_unit("test_osmo")
                    .expect("hardcoded \"test_osmo\" denom should be known")
                    .id(),
                asset::Cache::with_known_assets()
                    .get_unit("test_btc")
                    .expect("hardcoded \"test_btc\" denom should be known")
                    .id(),
            ]),
            max_hops: 4,
        }
    }
}

impl RoutingParams {
    /// Like `Default::default()`, but extends the default fixed candidates with the given list.
    ///
    /// If you want to _set_ the fixed candidates, just use `..Default::default()`.
    pub fn default_with_extra_candidates(iter: impl IntoIterator<Item = asset::Id>) -> Self {
        let mut params = Self::default();
        Arc::get_mut(&mut params.fixed_candidates)
            .expect("just created unique ref")
            .extend(iter);
        params
    }

    /// Clamps the spill price to the price limit and returns whether or not it was clamped.
    pub fn clamp_to_limit(&self, spill_price: Option<U128x128>) -> (Option<U128x128>, bool) {
        match (spill_price, self.price_limit) {
            (Some(spill_price), Some(price_limit)) => {
                if spill_price > price_limit {
                    (Some(price_limit), true)
                } else {
                    (Some(spill_price), false)
                }
            }
            (Some(spill_price), None) => (Some(spill_price), false),
            (None, Some(price_limit)) => (Some(price_limit), true),
            (None, None) => (None, false),
        }
    }
}
