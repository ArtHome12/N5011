/* ===============================================================================
Bot to support Telegram channel of 2:5011 Fidonet
Settings. 18 March 2021.
----------------------------------------------------------------------------
Licensed under the terms of the GPL version 3.
http://www.gnu.org/licenses/gpl-3.0.html
Copyright (c) 2020 by Artem Khomenko _mag12@yahoo.com.
=============================================================================== */

use once_cell::sync::OnceCell;

// Admin ID from environment
static ADMINS: OnceCell<Admins> = OnceCell::new();

static INTERVAL: OnceCell<u32> = OnceCell::new();

struct Admins {
   admin1: i32,
   admin2: i32,
}

pub fn is_admin(user_id: i32) -> bool {
   if let Some(a) = ADMINS.get() {
      a.admin1 == user_id || a.admin2 == user_id
   } else {
      false
   }
}

pub fn set_admins(admin1: i32, admin2: i32) -> Result<(), ()> {
   let a = Admins {
      admin1,
      admin2,
   };

   ADMINS.set(a).map_err(|_| ())
}

pub fn set_interval(v: i32) -> Result<(), ()> {
   INTERVAL.set(v as u32).map_err(|_| ())
}

pub fn interval() -> u32 {
   *INTERVAL.get_or_init(|| 3600)
}