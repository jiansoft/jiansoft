use anyhow::{Result};
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::{bot, crawler::twse, logging, util};

#[derive(Serialize, Deserialize)]
struct HolidayScheduleResponse {
    pub stat: Option<String>,
    pub date: String,
    pub data: Vec<Vec<String>>,
    #[serde(rename = "queryYear")]
    pub query_year: i64,
    pub total: i64,
}

pub async fn visit(year: i32) -> Result<Vec<NaiveDate>> {
    let now = Local::now();
    let url = format!(
        "https://www.{host}/rwd/zh/holidaySchedule/holidaySchedule?date={year}&response=json&_={time}",
        host = twse::HOST,
        year = year,
        time = now.timestamp_millis()
    );
    let res = util::http::get_use_json::<HolidayScheduleResponse>(&url).await?;
    let mut result: Vec<NaiveDate> = Vec::with_capacity(32);
    let stat = match res.stat {
        None => {
            report_error("HolidaySchedule.res.Stat is None").await;
            return Ok(result);
        }
        Some(stat) => stat.to_uppercase(),
    };

    if stat != "OK" {
        report_error("HolidaySchedule.res.Stat is not ok").await;
        return Ok(result);
    }

    for date_info in res
        .data
        .iter()
        .filter(|d| d.len() >= 3 && !d[2].contains("開始交易"))
    {
        if let Ok(d) = NaiveDate::parse_from_str(&date_info[0], "%Y-%m-%d") {
            result.push(d);
        }
    }

    Ok(result)
}

async fn report_error(message: &str) {
    if let Err(why) = bot::telegram::send(message).await {
        logging::error_file_async(format!("Failed to send because {:?}", why));
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::SHARE;
    use crate::logging;
    use chrono::Datelike;
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_visit() {
        dotenv::dotenv().ok();
        SHARE.load().await;
        logging::debug_file_async("開始 visit".to_string());
        let now = Local::now();
        match visit(now.date_naive().year()).await {
            Ok(list) => {
                dbg!(&list);
                logging::debug_file_async(format!("list:{:#?}", list));
            }
            Err(why) => {
                logging::debug_file_async(format!("Failed to visit because: {:?}", why));
            }
        }

        logging::debug_file_async("結束 visit".to_string());
    }
}
