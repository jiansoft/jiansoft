use crate::internal::{
    backfill::net_asset_value_per_share, crawler::yahoo, database::model, logging, nosql,
    util::datetime,
};
use anyhow::*;
use chrono::{Datelike, Duration, Local};
use core::result::Result::Ok;
use rust_decimal::Decimal;

/// 將未有上季度財報的股票，到雅虎財經下載後回寫到 financial_statement 表
pub async fn execute() -> Result<()> {
    let cache_key = "financial_statement::yahoo";
    let is_jump = nosql::redis::CLIENT.get_bool(cache_key).await?;
    if is_jump {
        return Ok(());
    }

    let previous_quarter = Local::now() - Duration::days(125);
    let year = previous_quarter.year();
    let quarter = datetime::month_to_quarter(previous_quarter.month());
    let stocks = model::stock::fetch_stocks_without_financial_statement(year, quarter).await?;
    let mut success_update_count = 0;
    for mut stock in stocks {
        if stock.is_preference_shares() {
            continue;
        }

        let profile = match yahoo::profile::visit(&stock.stock_symbol).await {
            Ok(profile) => profile,
            Err(why) => {
                logging::error_file_async(format!(
                    "Failed to yahoo::profile::visit because {:?}",
                    why
                ));
                continue;
            }
        };

        if year != profile.year || quarter != profile.quarter {
            logging::warn_file_async(format!(
                "the year or quarter retrieved from Yahoo is inconsistent with the current one. current year:{} ,quarter:{} {:#?}",
                year, quarter, profile
            ));
            continue;
        }

        let fs = model::financial_statement::Entity::from(profile);

        if let Err(why) = fs.upsert().await {
            logging::error_file_async(format!("Failed to upsert because {:?}", why));
            continue;
        }

        logging::info_file_async(format!(
            "financial_statement upsert executed successfully. \r\n{:#?}",
            fs
        ));

        //若原股票的每股淨值為零時，順便更新一下
        if stock.net_asset_value_per_share == Decimal::ZERO
            && fs.net_asset_value_per_share != Decimal::ZERO
        {
            stock.net_asset_value_per_share = fs.net_asset_value_per_share;
            if let Err(why) = net_asset_value_per_share::update(&stock).await {
                logging::error_file_async(format!(
                    "Failed to update_net_asset_value_per_share because {:?}",
                    why
                ));
            } else {
                logging::info_file_async(format!(
                    "update_net_asset_value_per_share executed successfully. \r\n{:#?}",
                    stock
                ));
            }
        }
        success_update_count += success_update_count;
    }

    if success_update_count > 0 {
        model::stock::Entity::update_last_eps().await?;
    }

    nosql::redis::CLIENT
        .set(cache_key, true, 60 * 60 * 24 * 7)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::cache::SHARE;
    use crate::internal::logging;

    #[tokio::test]
    async fn test_execute() {
        dotenv::dotenv().ok();
        SHARE.load().await;
        logging::debug_file_async("開始 execute".to_string());

        match execute().await {
            Ok(_) => {}
            Err(why) => {
                logging::debug_file_async(format!("Failed to execute because {:?}", why));
            }
        }

        logging::debug_file_async("結束 execute".to_string());
    }
}
