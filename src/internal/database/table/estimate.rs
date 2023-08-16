use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use sqlx::postgres::PgQueryResult;

use crate::internal::database;

#[derive(sqlx::FromRow, Debug, Default)]
pub struct Estimate {
    pub date: NaiveDate, // 使用 chrono 庫來處理日期和時間
    pub last_daily_quote_date: String,
    pub security_code: String,
    pub name: String,
    pub closing_price: f64,
    pub percentage: f64,
    pub cheap: f64,
    pub fair: f64,
    pub expensive: f64,
    pub price_cheap: f64,
    pub price_fair: f64,
    pub price_expensive: f64,
    pub dividend_cheap: f64,
    pub dividend_fair: f64,
    pub dividend_expensive: f64,
    pub year_count: i32,
    pub index: i32,
}

impl Estimate {
    pub async fn insert(date: NaiveDate) -> Result<PgQueryResult> {
        let years: Vec<i32> = (0..10).map(|i| date.year() - i).collect();
        let years_str: Vec<String> = years.iter().map(|&year| year.to_string()).collect();
        let years_str = years_str.join(",");

        let sql = format!(
            r#"
WITH stocks AS (
  SELECT stock_symbol
  FROM stocks AS c
  WHERE c."SuspendListing" = false
  AND c.stock_industry_id IN (
    SELECT stock_industry.stock_industry_id
    FROM stock_industry
  )
),
price as (
	SELECT
        s.stock_symbol AS "SecurityCode",
        COUNT(DISTINCT dq."year") AS year_count,
        MIN(dq."ClosingPrice") AS price_cheap,
        AVG(dq."ClosingPrice") AS price_fair,
        MAX(dq."ClosingPrice") AS price_expensive
    FROM
        stocks s
    INNER JOIN
        "DailyQuotes" dq ON s.stock_symbol = dq."SecurityCode"
    WHERE
        dq."year" in ({1})
        AND dq."ClosingPrice" > 0
    GROUP BY
        s.stock_symbol
),
dividend as (
	select security_code,
		avg("sum") * 15 as dividend_cheap,
		avg("sum") * 20 as dividend_fair,
		avg("sum") * 30 as dividend_expensive
	from dividend
	where "year" in ({1})
	group by security_code
)
insert into estimate (
	security_code, "date", percentage, closing_price, cheap, fair, expensive, price_cheap,
	price_fair, price_expensive, dividend_cheap, dividend_fair, dividend_expensive, year_count
)
select dq."SecurityCode",
	dq."Date",
	(((dq."ClosingPrice" / ((price_cheap + dividend_cheap) / 2))) * 100) as percentage,
	dq."ClosingPrice",
	(price_cheap + dividend_cheap) / 2                                   as cheap,
	(price_fair + dividend_fair) / 2                                     as fair,
	(price_expensive + dividend_expensive) / 2                           as expensive,
	price_cheap,
	price_fair,
	price_expensive,
	dividend_cheap,
	dividend_fair,
	dividend_expensive,
	year_count
from stocks AS c
inner join "DailyQuotes" as dq on "c".stock_symbol = dq."SecurityCode" and dq."Date" = '{0}'
inner join price on dq."SecurityCode" = price."SecurityCode"
inner join dividend on dq."SecurityCode" = dividend.security_code
ON CONFLICT (date,security_code) DO NOTHING;
"#,
            date.format("%Y-%m-%d"),
            years_str
        );

        sqlx::query(&sql)
            .execute(database::get_connection())
            .await
            .context("Failed to insert estimate from database")
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use crate::internal::logging;

    use super::*;

    #[tokio::test]
    async fn test_insert() {
        dotenv::dotenv().ok();
        logging::debug_file_async("開始 insert".to_string());
        let current_date = Local::now().date_naive();
        match Estimate::insert(current_date).await {
            Ok(r) => logging::info_file_async(format!("{:#?}", r)),
            Err(why) => {
                logging::debug_file_async(format!("Failed to insert because {:?}", why));
            }
        }

        logging::debug_file_async("結束 insert".to_string());
    }
}
