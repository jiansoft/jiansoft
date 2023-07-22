use std::{env, future::Future, result::Result::Ok};

use anyhow::*;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::internal::{backfill, bot, crawler, logging, reminder};

// Constants for logging messages
const BACKFILL_FINANCIAL_STATEMENT_ANNUAL: &str = "backfill::financial_statement::annual::execute";
const BACKFILL_FINANCIAL_STATEMENT_QUARTER: &str =
    "backfill::financial_statement::quarter::execute";
const BACKFILL_NET_ASSET_VALUE_EMERGING: &str =
    "backfill::net_asset_value_per_share::emerging::execute";
const BACKFILL_NET_ASSET_VALUE_ZERO_VALUE: &str =
    "backfill::net_asset_value_per_share::zero_value::execute";
const BACKFILL_INTERNATIONAL_SECURITIES_IDENTIFICATION_NUMBER: &str =
    "backfill::international_securities_identification_number::execute";
const BACKFILL_DELISTED_COMPANY: &str = "backfill::delisted_company::execute";
const BACKFILL_REVENUE: &str = "backfill::revenue::execute";
const BACKFILL_TAIWAN_CAPITALIZATION_WEIGHTED_STOCK_INDEX: &str =
    "backfill::taiwan_capitalization_weighted_stock_index::execute";
const BACKFILL_QUOTES: &str = "backfill::quotes::execute";
const BACKFILL_DIVIDEND: &str = "backfill::dividend::execute";
const BACKFILL_STOCK_WEIGHT: &str = "backfill::stock_weight::execute";

/// 啟動排程
pub async fn start() {
    if let Err(why) = run_cron().await {
        logging::error_file_async(format!("Failed to run_cron because {:?}", why));
    }

    let msg = format!(
        "StockCrawler 已啟動\r\nRust OS/Arch: {}/{}\r\n",
        env::consts::OS,
        env::consts::ARCH
    );

    if let Err(err) = bot::telegram::send(&msg).await {
        logging::error_file_async(format!("Failed to send telegram message because {:?}", err));
    }
}

async fn run_and_log_task<F, Fut>(task_name: &str, task: F)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), Error>>,
{
    logging::info_file_async(format!("開始 {}", task_name));
    match task().await {
        Ok(_) => {
            logging::info_file_async(format!("{} executed successfully.", task_name));
        }
        Err(why) => {
            logging::error_file_async(format!("Failed to {} because {:?}", task_name, why));
        }
    }
    logging::info_file_async(format!("結束 {}", task_name));
}

pub async fn run_cron() -> Result<()> {
    let sched = JobScheduler::new().await?;
    //                 sec  min   hour   day of month   month   day of week   year
    //let expression = "0   30   9,12,15     1,15       May-Aug  Mon,Wed,Fri  2018/2";
    // UTC 時間

    // 01:00
    let one_am_job = Job::new_async("0 0 17 * * *", |_uuid, _l| {
        Box::pin(async {
            //更新台股季度財報
            run_and_log_task(
                BACKFILL_FINANCIAL_STATEMENT_QUARTER,
                backfill::financial_statement::quarter::execute,
            )
            .await;

            //更新興櫃股票的每股淨值
            run_and_log_task(
                BACKFILL_NET_ASSET_VALUE_EMERGING,
                backfill::net_asset_value_per_share::emerging::execute,
            )
            .await;
        })
    })?;
    sched.add(one_am_job).await?;

    // 03:00
    let three_am_job = Job::new_async("0 0 19 * * *", |_uuid, _l| {
        Box::pin(async {
            //從yahoo取得每股淨值數據，將未下市但每股淨值為零的股票更新其數據
            run_and_log_task(
                BACKFILL_NET_ASSET_VALUE_ZERO_VALUE,
                backfill::net_asset_value_per_share::zero_value::execute,
            )
            .await;

            //更新台股年度財報
            run_and_log_task(
                BACKFILL_FINANCIAL_STATEMENT_ANNUAL,
                backfill::financial_statement::annual::execute,
            )
            .await;
        })
    })?;
    sched.add(three_am_job).await?;

    // 05:00
    let five_am_job = Job::new_async("0 0 21 * * *", |_uuid, _l| {
        Box::pin(async {
            //取得台股的營收
            run_and_log_task(BACKFILL_REVENUE, backfill::revenue::execute).await;

            //取得台股國際證券識別碼
            run_and_log_task(
                BACKFILL_INTERNATIONAL_SECURITIES_IDENTIFICATION_NUMBER,
                backfill::international_securities_identification_number::execute,
            )
            .await;

            //更新下市的股票
            run_and_log_task(
                BACKFILL_DELISTED_COMPANY,
                backfill::delisted_company::execute,
            )
            .await;

            //更新股票權值佔比
            run_and_log_task(BACKFILL_STOCK_WEIGHT, backfill::stock_weight::execute).await;
        })
    })?;
    sched.add(five_am_job).await?;

    // 08:00
    let eight_am_job = Job::new_async("0 0 0 * * *", |_uuid, _l| {
        Box::pin(async {
            //提醒本日除權息的股票
            reminder::ex_dividend::execute().await;
        })
    })?;
    sched.add(eight_am_job).await?;

    // 15:00
    let three_pm_job = Job::new_async("0 0 7 * * *", |_uuid, _l| {
        Box::pin(async {
            //更新台股收盤指數
            run_and_log_task(
                BACKFILL_TAIWAN_CAPITALIZATION_WEIGHTED_STOCK_INDEX,
                backfill::taiwan_capitalization_weighted_stock_index::execute,
            )
            .await;
        })
    })?;
    sched.add(three_pm_job).await?;

    // 15:01
    let three_one_pm_job = Job::new_async("0 1 7 * * *", |_uuid, _l| {
        Box::pin(async {
            //取得收盤報價數據
            run_and_log_task(BACKFILL_QUOTES, backfill::quote::execute).await;
        })
    })?;
    sched.add(three_one_pm_job).await?;

    // 21:00
    let nine_pm_job = Job::new_async("0 0 13 * * *", |_uuid, _l| {
        Box::pin(async {
            //資料庫內尚未有年度配息數據的股票取出後向第三方查詢後更新回資料庫
            run_and_log_task(BACKFILL_DIVIDEND, backfill::dividend::execute).await;
        })
    })?;
    sched.add(nine_pm_job).await?;

    let every_minute = Job::new_async("0 * * * * *", |_uuid, _l| {
        Box::pin(async {
            crawler::free_dns::update().await;
        })
    })?;
    sched.add(every_minute).await?;

    sched.start().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use tokio::time::{sleep, Duration};

    // 注意這個慣用法：在 tests 模組中，從外部範疇匯入所有名字。
    use super::*;

    async fn run() -> Result<()> {
        let sched = JobScheduler::new().await?;
        let every_minute = Job::new_async("* * * * * *", |_uuid, _l| {
            Box::pin(async move {
                println!("_uuid {:?} now: {:?}", _uuid, chrono::Local::now());
                dbg!("_uuid {:?} now: {:?}", _uuid, chrono::Local::now());
                logging::debug_file_async(format!(
                    "_uuid {:?} now: {:?}",
                    _uuid,
                    chrono::Local::now()
                ));
            })
        })?;
        sched.add(every_minute).await?;

        sched.start().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_split() {
        dotenv::dotenv().ok();
        run().await.expect("TODO: panic message");
        sleep(Duration::from_secs(240)).await;
        //loop {}
        //println!("split: {:?}, elapsed time: {:?}", result, end);
    }
}
