/* ===============================================================================
Bot to support Telegram channel of 2:5011 Fidonet
Database module. 12 March 2021.
----------------------------------------------------------------------------
Licensed under the terms of the GPL version 3.
http://www.gnu.org/licenses/gpl-3.0.html
Copyright (c) 2020 by Artem Khomenko _mag12@yahoo.com.
=============================================================================== */

use once_cell::sync::OnceCell;

use crate::settings as set;

// Database
pub static DB: OnceCell<tokio_postgres::Client> = OnceCell::new();

struct User {
   // id: i32,
   descr: Option<String>,
   addr: Option<String>,
   last_seen: i32,
}

pub enum AnnouncementError {
   SmallInterval, // too little time has passed since the last call
   NoneAddr, // no information and must be requested from an external source
}

pub type AnnouncementResult = Result<String, AnnouncementError>;

// Announcement text for the user, if necessary
pub async fn announcement(user_id: i64, time: i32) -> AnnouncementResult {

   match load_user(user_id).await {
      Some(user) => {
         // No info - no announcement
         if user.addr.is_none() {
            return Err(AnnouncementError::NoneAddr);
         }

         // If enough time has passed
         if (time - user.last_seen) as u32 > set::interval() {
            update_user_time(user_id, time).await;
            Ok(format!("{} {}", user.addr.unwrap(), user.descr.unwrap_or_default()))
         } else {
            Err(AnnouncementError::SmallInterval)
         }
      }
      None => {
         // Remember a new user
         save_new_user(user_id, time).await;
         Err(AnnouncementError::NoneAddr)
      }
   }
}

// Создаёт таблицы, если её ещё не существует
pub async fn check_database() {
   // Получаем клиента БД
   let client = DB.get().unwrap();

   // Выполняем запрос
   let rows = client.query("SELECT table_name FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_NAME='users'", &[]).await.unwrap();

   // Если таблица не существует, создадим её
   if rows.is_empty() {
      log::info!("Create database");

      let query = client.batch_execute("CREATE TABLE users (
         PRIMARY KEY (user_id),
         user_id        BIGINT         NOT NULL,
         descr          VARCHAR(100),
         addr           VARCHAR(100),
         last_seen      INTEGER        NOT NULL
      );

      CREATE TABLE settings (announcement_delta INTEGER);
      INSERT INTO settings (announcement_delta) VALUES (30);
      ")
      .await;

      if let Err(e) = query {
         log::info!("check_database create error: {}", e)
      }
   } else {
      log::info!("Database exists");
   }

   // Init settings
   let data = client.query_one("SELECT announcement_delta FROM settings", &[]).await;

   if let Err(_) = data.map_err(|_| ()).and_then(|row| set::init_interval(row.get(0))) {
      log::info!("check_database() Error load settings");
   }

   log::info!("Interval for announcements {} sec", set::interval());
}

async fn load_user(id: i64) -> Option<User> {
   let client = DB.get().unwrap();
   let query = client.query("SELECT descr, addr, last_seen FROM users WHERE user_id=$1::BIGINT", &[&id]).await;

   match query {
      Ok(data) => {
         match data.len() {
            1 => Some(User{
               // id,
               descr: data[0].get(0),
               addr: data[0].get(1),
               last_seen: data[0].get(2),
            }),
            _ => None,
         }

      }
      Err(e) => {
         log::info!("load_user error: {}, {}", id, e);
         None
      }
   }
}

pub async fn update_user_time(id: i64, time: i32) {
   let client = DB.get().unwrap();
   let query = client.execute("UPDATE users SET last_seen = $1::INTEGER WHERE user_id = $2::BIGINT", &[&time, &id]).await;

   match query {
      Ok(1) => (),
      Ok(n) => log::info!("update_user_time error: {}, {} - updated {} records", id, time, n),
      Err(e) => log::info!("update_user_time error: {}, {} - {}", id, time, e),
   }
}

pub async fn save_new_user(id: i64, time: i32) {
   let client = DB.get().unwrap();
   let query = client.execute("INSERT INTO users (user_id, last_seen) VALUES ($1::BIGINT, $2::INTEGER)", &[&id, &time]).await;

   match query {
      Ok(1) => (),
      Ok(n) => log::info!("update_user_time error: {}, {} - updated {} records", id, time, n),
      Err(e) => log::info!("update_user_time error: {}, {} - {}", id, time, e),
   }
}

pub async fn user_descr(id: i64) -> String {
   match load_user(id).await {
      Some(user) => if user.addr.is_some() {format!("{}\n{}", user.addr.unwrap(), user.descr.unwrap_or_default())} else {user.descr.unwrap_or_default()},
      None => String::default(),
   }
}

pub async fn update_user_descr(id: i64, descr: &str) {
   let client = DB.get().unwrap();
   let query = client.execute("UPDATE users SET descr = $1::VARCHAR(100) WHERE user_id = $2::BIGINT", &[&descr, &id]).await;

   match query {
      Ok(1) => (),
      Ok(n) => log::info!("update_user_descr error: {}, {} - updated {} records", id, descr, n),
      Err(e) => log::info!("update_user_descr error: {}, {} - {}", id, descr, e),
   }
}

pub async fn update_interval(i: i32) -> Result<(), ()> {
   let client = DB.get().unwrap();
   let query = client.execute("UPDATE settings SET announcement_delta = $1::INTEGER", &[&i]).await;

   match query {
      Ok(1) => Ok(()),
      Ok(n) => {log::info!("update_interval error: {} - updated {} records", i, n); Err(())},
      Err(e) => {log::info!("update_interval error: {} - {}", i, e); Err(())},
   }
}

pub async fn update_user_addr(id: i64, addr: &str) {
   let client = DB.get().unwrap();
   let query = client.execute("UPDATE users SET addr = $1::VARCHAR(100) WHERE user_id = $2::BIGINT", &[&addr, &id]).await;

   match query {
      Ok(1) => (),
      Ok(n) => log::info!("update_user_addr error: {}, {} - updated {} records", id, addr, n),
      Err(e) => log::info!("update_user_addr error: {}, {} - {}", id, addr, e),
   }
}

