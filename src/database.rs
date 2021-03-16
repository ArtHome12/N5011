/* ===============================================================================
Bot to support Telegram channel of 2:5011 Fidonet
Database module. 12 March 2021.
----------------------------------------------------------------------------
Licensed under the terms of the GPL version 3.
http://www.gnu.org/licenses/gpl-3.0.html
Copyright (c) 2020 by Artem Khomenko _mag12@yahoo.com.
=============================================================================== */

use once_cell::sync::OnceCell;
use std::sync::{Mutex, Arc};
use std::collections::{HashMap, };
use serde_derive::{Serialize, Deserialize, };
use std::fs;


// Storage
pub static DB: OnceCell<Arc<Mutex<Storage>>> = OnceCell::new();

// Data structure
#[derive(Serialize, Deserialize)]
pub struct User {
   pub descr: String,
   pub last_seen: i32,
}

#[derive(Serialize, Deserialize)]
pub struct Users {
   announcement_delta: i32,
   users: HashMap<String, User>,
}

impl Users {
   pub fn new(filename: &str) -> Self {
      log::info!("Load1");
      if let Ok(data) = fs::read_to_string(filename) {
         log::info!("Load: {}", data);
         toml::from_str(&data).unwrap()
      } else {
         Default::default()
      }
   }
}

impl Default for Users {
   fn default() -> Self {
      Self {
         announcement_delta: 5, //60 * 60 * 24,
         users: HashMap::new(),
      }
   }
}

pub struct Storage {
   filename: String,
   users: Users,
}

impl Storage {
   // Load or create new data
   pub fn new(filename: &str) -> Self {
      Self {
         filename: String::from(filename),
         users: Users::new(filename),
      }
   }

   // Save data
   fn save(&self) {
      match toml::to_string(&self.users) {
         Ok(str_data) => {
            log::info!("Save: {}", str_data);
            if let Err(e) = fs::write(self.filename.as_str(), str_data) {
               log::info!("Save file error: {}", e);
            }
         }
         Err(e) => log::info!("Save toml error: {}", e),
      }
      log::info!("Save2");
   }

   // Announcement text for the user, if necessary
   pub fn announcement(&mut self, user_id: i32, time: i32, def_descr: &str) -> Option<String> {
      // TOML requirement
      let user_id_str = user_id.to_string();

      match self.users.users.get_mut(&user_id_str) {
         Some(user) => {
            // If enough time has passed
            if time - user.last_seen > self.users.announcement_delta {
               user.last_seen = time;
               let descr = user.descr.clone();
               self.save();
               Some(descr)
            } else {
               None
            }
         }
         None => {
            // Remember a new user
            let user = User {
               descr: String::from(def_descr),
               last_seen: time,
            };

            self.users.users.insert(user_id_str, user);
            self.save();
            None
         }
      }
   }
}