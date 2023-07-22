use sqlx::FromRow;

pub(crate) mod net_asset_value_per_share;
pub(crate) mod suspend_listing;
pub(crate) mod weight;

#[derive(FromRow, Debug)]
pub struct SymbolAndName {
    pub stock_symbol: String,
    pub name: String,
}
