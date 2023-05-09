use crate::internal::{database::DB, logging};
use anyhow::Result;
use chrono::{DateTime, Local};
use sqlx::{postgres::PgRow, QueryBuilder, Row};
use std::collections::HashMap;

#[rustfmt::skip]
#[derive(sqlx::Type, sqlx::FromRow, Debug)]
pub struct Entity {
    pub word_id: i64,
    pub word: String,
    pub created_time: DateTime<Local>,
    pub updated_time: DateTime<Local>,
}

impl Entity {
    pub fn new(word: String) -> Self {
        Entity {
            word_id: Default::default(),
            word,
            created_time: Local::now(),
            updated_time: Local::now(),
        }
    }

    pub fn clone(&self) -> Self {
        Entity {
            word_id: self.word_id,
            word: self.word.to_string(),
            created_time: self.created_time,
            updated_time: self.updated_time,
        }
    }

    /// 新增數據到資料庫後回傳新增的 word_id
    pub async fn insert(&mut self) -> Result<i64> {
        /*let mut transaction = DB.pool.begin().await?;
        let query = "insert into company_word (word, created_time, updated_time) values ($1,$2,$3)
            on conflict (word) do update set
            word = excluded.word,
            updated_time = excluded.updated_time
            returning word_id;";

        match sqlx::query_as::<Postgres, (i64,)>(query)
            .bind(&self.word)
            .bind(self.created_time)
            .bind(self.updated_time)
            .fetch_one(&mut transaction)
            .await
        {
            Ok((last_insert_id,)) => {
                transaction.commit().await?;
                self.word_id = last_insert_id;
                Ok(last_insert_id)
            }
            Err(why) => {
                transaction.rollback().await?;
                Err(anyhow!("{:?}", why))
            }
        }*/

        let query = "INSERT INTO company_word (word, created_time, updated_time)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (word) DO UPDATE SET
                    word = EXCLUDED.word,
                    updated_time = EXCLUDED.updated_time
                 RETURNING word_id";

        let row = sqlx::query(query)
            .bind(&self.word)
            .bind(self.created_time)
            .bind(self.updated_time)
            .fetch_one(&DB.pool)
            .await?;

        let word_id: i64 = row.try_get("word_id")?;
        self.word_id = word_id;
        Ok(word_id)
    }

    /// 從資料表中取得公司代碼、名字拆字後的數據
    pub async fn list_by_word(words: &Vec<String>) -> Option<Vec<Entity>> {
        let mut query_builder =
            QueryBuilder::new("select word_id,word,created_time,updated_time from company_word");

        if !words.is_empty() {
            query_builder.push(" where word = any(");
            query_builder.push_bind(words);
            query_builder.push(")");
        }

        match query_builder
            .build()
            .try_map(|row: PgRow| {
                let created_time = row.try_get("created_time")?;
                let updated_time = row.try_get("updated_time")?;
                let word_id = row.try_get("word_id")?;
                let word = row.try_get("word")?;
                Ok(Entity {
                    word_id,
                    word,
                    created_time,
                    updated_time,
                })
            })
            .fetch_all(&DB.pool)
            .await
        {
            Ok(result) => Some(result),
            Err(why) => {
                logging::error_file_async(format!(
                    "Failed to fetch entities from the database: {:?}",
                    why
                ));
                None
            }
        }
    }
}

impl Clone for Entity {
    fn clone(&self) -> Self {
        self.clone()
    }
}

impl Default for Entity {
    fn default() -> Self {
        Self::new("".to_string())
    }
}

/// 將 vec 轉成 hashmap
pub fn vec_to_hashmap_key_using_word(entities: Option<Vec<Entity>>) -> HashMap<String, Entity> {
    let mut stock_words = HashMap::new();
    if let Some(list) = entities {
        for e in list {
            stock_words.insert(e.word.to_string(), e);
        }
    }

    stock_words
}

/*/// 將 vec 轉成 hashmap
fn vec_to_hashmap(v: Option<Vec<Entity>>) -> HashMap<String, Entity> {
    v.unwrap_or_default()
        .iter()
        .fold(HashMap::new(), |mut acc, e| {
            acc.insert(e.word.to_string(), e.clone());
            acc
        })
}*/

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::logging;
    use crate::internal::util;
    use std::time::Instant;

    #[tokio::test]
    async fn test_vec_to_hashmap() {
        dotenv::dotenv().ok();
        let mut entities: Vec<Entity> = Vec::new();
        for i in 0..1000000 {
            entities.push(Entity {
                word_id: 0,
                word: format!("word_{}", i),
                created_time: Default::default(),
                updated_time: Default::default(),
            });
        }

        let start1 = Instant::now();
        let _hm1 = vec_to_hashmap_key_using_word(Some(entities.clone()));
        let elapsed1 = start1.elapsed().as_millis();

        /*let start2 = Instant::now();
        let hm2 = vec_to_hashmap(Some(entities.clone()));
        let elapsed2 = start2.elapsed().as_millis();*/

        println!("Method 1 elapsed time: {}", elapsed1);
        //println!("Method 2 elapsed time: {}", elapsed2);
        //println!("HashMap length: {} {}", hm1.len(), hm2.len());
    }

    /*    #[tokio::test]
        async fn test_split_1() {
            dotenv::dotenv().ok();
            let chinese_word = "台積電";
            let start = Instant::now();
            let result = split_v1(chinese_word);
            let end = start.elapsed();
            println!("split: {:?}, elapsed time: {:?}", result, end);
        }
    */

    #[tokio::test]
    async fn test_insert() {
        dotenv::dotenv().ok();
        let mut e = Entity::new("小一".to_string());
        match e.insert().await {
            Ok(word_id) => {
                logging::info_file_async(format!("word_id:{} e:{:#?}", word_id, &e));
                let _ = sqlx::query("delete from company_word where word_id = $1;")
                    .bind(word_id)
                    .execute(&DB.pool)
                    .await;
            }
            Err(why) => {
                logging::error_file_async(format!("because:{:?}", why));
            }
        }
    }

    #[tokio::test]
    async fn test_list_by_word() {
        dotenv::dotenv().ok();
        let word = util::text::split("台積電");
        let entities = Entity::list_by_word(&word).await;
        logging::info_file_async(format!("entities:{:#?}", entities));
        logging::info_file_async(format!(
            "word:{:#?}",
            vec_to_hashmap_key_using_word(entities)
        ));
    }
}
