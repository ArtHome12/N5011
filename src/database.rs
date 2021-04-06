/* ===============================================================================
Bot to support Telegram channel of 2:5011 Fidonet
Database module. 12 March 2021.
----------------------------------------------------------------------------
Licensed under the terms of the GPL version 3.
http://www.gnu.org/licenses/gpl-3.0.html
Copyright (c) 2020 by Artem Khomenko _mag12@yahoo.com.
=============================================================================== */

use once_cell::sync::OnceCell;
use serde::Deserialize;
use std::cmp::Ordering;
use reqwest::Client;

use crate::settings as set;

// Database
pub static DB: OnceCell<tokio_postgres::Client> = OnceCell::new();

struct User {
   descr: Option<String>,
   addr: Option<String>,
   last_seen: i32,
   num_short_announcements: i32,
}

// Announcement text for the user, if necessary
pub async fn announcement(user_id: i64, time: i32) -> Option<String> {

   match load_user(user_id).await {
      Some(user) => {
         // If enough time has passed
         if (time - user.last_seen) as u32 > set::interval() {
            update_user_time(user_id, time).await;

            let mut addr = user.addr.unwrap_or(String::default());

            if user.num_short_announcements >= 12 {
               reset_num_short_announcements(user_id).await;
            } else {
               addr = addr.split(",").take(2).collect();
            };

            // Ask about updates
            tokio::spawn(request_addr(user_id));
            
            let res = if addr == "" {
               format!("БОФА {}", user.descr.unwrap_or_default())
            } else {
               format!("{} {}", addr, user.descr.unwrap_or_default())
            };

            Some(res)
         } else {
            // To small time elapsed
            None
         }
      }
      None => {
         // Remember a new user
         save_new_user(user_id, time).await;
         None
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
         last_seen      INTEGER        NOT NULL,
         num_short_announcements INTEGER NOT NULL
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
   let query = client.query("SELECT descr, addr, last_seen, num_short_announcements FROM users WHERE user_id=$1::BIGINT", &[&id]).await;

   match query {
      Ok(data) => {
         match data.len() {
            1 => Some(User{
               // id,
               descr: data[0].get(0),
               addr: data[0].get(1),
               last_seen: data[0].get(2),
               num_short_announcements: data[0].get(3),
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
   let query = client.execute("UPDATE users SET last_seen = $1::INTEGER, num_short_announcements = num_short_announcements + 1 WHERE user_id = $2::BIGINT", &[&time, &id]).await;

   match query {
      Ok(1) => (),
      Ok(n) => log::info!("update_user_time error: {}, {} - updated {} records", id, time, n),
      Err(e) => log::info!("update_user_time error: {}, {} - {}", id, time, e),
   }
}

pub async fn reset_num_short_announcements(id: i64) {
   let client = DB.get().unwrap();
   let query = client.execute("UPDATE users SET num_short_announcements = 0 WHERE user_id = $1::BIGINT", &[&id]).await;

   match query {
      Ok(1) => (),
      Ok(n) => log::info!("reset_num_short_announcements error: {} - updated {} records", id, n),
      Err(e) => log::info!("reset_num_short_announcements error: {} - {}", id, e),
   }
}

pub async fn save_new_user(id: i64, time: i32) {
   let client = DB.get().unwrap();
   let query = client.execute("INSERT INTO users (user_id, last_seen, num_short_announcements) VALUES ($1::BIGINT, $2::INTEGER, 0)", &[&id, &time]).await;

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

#[derive(Deserialize)]
struct Node {
   pub addr: String,
   pub name: String,
   pub telegram_name: Option<String>,
   pub telegram_login: Option<String>,
   pub user_id: i64,
}

impl Node {
   pub fn addr_struct(&self) -> (usize, usize) {
      let v: Vec<&str> = self.addr.split(&['/', '.'][..]).collect();
      match v.len() {
         2 => (v[1].parse().unwrap_or(0), 0),
         3 => (v[1].parse().unwrap_or(0), v[2].parse().unwrap_or(0)),
         _ => (0, 0)
      }
   }
}

impl PartialOrd for Node {
   fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
      // Sort by node number and point number
      let a = self.addr_struct();
      let b = other.addr_struct();
      if a.1 == 0 && b.1 > 0 {
         Some(Ordering::Less)
      } else if a.1 > 0 && b.1 == 0 {
         Some(Ordering::Greater)
      } else {
         a.partial_cmp(&b)
      }
   }
}

impl PartialEq for Node {
   fn eq(&self, other: &Self) -> bool {
      self.addr == other.addr
   }
}

impl Eq for Node {}

impl Ord for Node {
   fn cmp(&self, other: &Self) -> Ordering {
      self.partial_cmp(other).unwrap()
   }
}

type Nodelist = Vec<Node>;

fn from_nodelist(mut nodelist: Nodelist) -> String {
   let name = if nodelist.len() > 0 {
      nodelist[0].name.clone()
   } else {
      return String::from("Ошибка, пустой нодлист");
   };

   nodelist.sort();

   let mut addrs = nodelist.iter().map(|i| i.addr.clone()).collect::<Vec<String>>();

   // Clip point .1 afer the node
   addrs.dedup_by(|a, b| a.starts_with(b.as_str()));

   // Remove repeated prefix
   let mut suffix = addrs.split_off(1).iter().map(|s| s.replace("2:5011/", "/")).collect::<Vec<String>>();
   addrs.append(&mut suffix);

   addrs.iter().fold(name, |acc, s| format!("{}, {}", acc, s))
}

async fn request_addr(user_id: i64) {
   let url = format!("https://guestl.info/grfidobot/api/v1/users/{}", user_id);

   let req = Client::new()
   .get(url)
   .basic_auth("arthome", Some("emminet"))
   .send()
   .await;

   match req {
      Ok(req) => {
         let body = req.json::<Nodelist>().await;
         match body {
            Ok(nodelist) => {
               let s = &from_nodelist(nodelist);
               log::info!("request_addr updated for {}: {}", user_id, s);
               update_user_addr(user_id, s).await
            },
            Err(e) => log::info!("body error for {}: {}", user_id, e),
         };
      }
      Err(e) => log::info!("req error for {}: {}", user_id, e),
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

