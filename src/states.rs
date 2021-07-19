/* ===============================================================================
Bot to support Telegram channel of 2:5011 Fidonet
Dialogue FSM. 16 March 2021.
----------------------------------------------------------------------------
Licensed under the terms of the GPL version 3.
http://www.gnu.org/licenses/gpl-3.0.html
Copyright (c) 2020 by Artem Khomenko _mag12@yahoo.com.
=============================================================================== */

use derive_more::From;
use teloxide_macros::{Transition, teloxide, };
use teloxide::{prelude::*,
   types::{ReplyMarkup, KeyboardButton, KeyboardMarkup, },
};
use std::convert::TryFrom;

use crate::database as db;
use crate::settings as set;


// FSM states
#[derive(Transition, From, Clone)]
pub enum Dialogue {
   Start(StartState),
   Command(CommandState),
   Origin(OriginState),
   Interval(IntervalState),
}

impl Default for Dialogue {
   fn default() -> Self {
       Self::Start(StartState { restarted: true })
   }
}

// Commands for bot
enum Command {
   Origin,  // change origin
   List, // List all users
   Interval, // Set time interval for announcements
}

impl TryFrom<&str> for Command {
   type Error = &'static str;

   fn try_from(s: &str) -> Result<Self, Self::Error> {
      match s {
         "Изменить ориджин" => Ok(Command::Origin),
         "Список" => Ok(Command::List),
         "Интервал" => Ok(Command::Interval),
         _ => Err("Неизвестная команда"),
      }
   }
}

impl From<Command> for String {
   fn from(c: Command) -> String {
      match c {
         Command::Origin => String::from("Изменить ориджин"),
         Command::List => String::from("Список"),
         Command::Interval => String::from("Интервал"),
      }
   }
}

// Frequently used menu
fn one_button_markup(label: &'static str) -> ReplyMarkup {
   let keyboard = vec![vec![KeyboardButton::new(label)]];
   let keyboard = KeyboardMarkup::new(keyboard)
   .resize_keyboard(true);

   ReplyMarkup::Keyboard(keyboard)
}


#[derive(Clone)]
pub struct StartState {
   restarted: bool,
}

#[teloxide(subtransition)]
async fn start(state: StartState, cx: TransitionIn<AutoSend<Bot>>, _ans: String,) -> TransitionOut<Dialogue> {
   // Extract user id
   let user = cx.update.from();
   if user.is_none() {
      cx.answer("Error, no user").await?;
      return next(StartState { restarted: false });
   }

   // For admin and regular users there is different interface
   let user_id = user.unwrap().id;
   let is_admin = set::is_admin(user_id);

   // Prepare menu
   let commands = if is_admin {
      vec![KeyboardButton::new(Command::Origin),
      // KeyboardButton::new(Command::List),
      KeyboardButton::new(Command::Interval),
      ]
   } else {
      vec![KeyboardButton::new(Command::Origin)]
   };

   let keyboard = KeyboardMarkup::new(vec![commands])
   .resize_keyboard(true);

   let markup = ReplyMarkup::Keyboard(keyboard);

   let info = String::from(if state.restarted { "Извините, бот был перезапущен.\n" } else {""});
   let info = info + "Добро пожаловать. Выберите команду на кнопке внизу";

   cx.answer(info)
   .reply_markup(markup)
   .await?;
   next(CommandState { user_id, is_admin })
}

#[derive(Clone)]
pub struct CommandState {
   user_id: i64,
   is_admin: bool,
}

#[teloxide(subtransition)]
async fn select_command(state: CommandState, cx: TransitionIn<AutoSend<Bot>>, ans: String,) -> TransitionOut<Dialogue> {
   // Parse text from user
   let command = Command::try_from(ans.as_str());
   if command.is_err() {
      cx.answer(format!("Неизвестная команда {}. Пожалуйста, выберите одну из команд внизу (если панель с кнопками скрыта, откройте её)", ans)).await?;

      // Stay in previous state
      return next(state)
   }

   // Handle commands
   match command.unwrap() {
      Command::Origin => {
         // Collect info about update
         let info = db::user_descr(state.user_id).await;
         let info = format!("Ваш текущий ориджин\n{}\nПожалуйста, введите текст для отображения после информации нодлиста\n Для отказа нажмите /", info);

         cx.answer(info)
         .reply_markup(one_button_markup("/"))
         .await?;

         next(OriginState { state })
      }

      Command::Interval => {
         let info = format!("Время с момента последнего сообщения пользователя для напоминания его адреса {} ч. Введите новый интервал в часах или / для отмены", set::interval() / 3600);

         cx.answer(info)
         .reply_markup(one_button_markup("/"))
         .await?;

         next(IntervalState { state })
      }
      _ => next(state),
   }
}

// #[derive(Generic)]
#[derive(Clone)]
pub struct OriginState {
   state: CommandState,
}

#[teloxide(subtransition)]
async fn origin(state: OriginState, cx: TransitionIn<AutoSend<Bot>>, ans: String,) -> TransitionOut<Dialogue> {
   let info = if ans == "/" {
      String::from("Ориджин не изменён")
   } else {
      // Save to database
      db::update_user_descr(state.state.user_id, &ans).await;

      format!("Ваш новый ориджин {} сохранён", ans)
   };

   cx.answer(info)
   .reply_markup(one_button_markup("В начало"))
   .await?;
   
   next(StartState { restarted: false })
}

// #[derive(Generic)]
#[derive(Clone)]
pub struct IntervalState {
   state: CommandState,
}

#[teloxide(subtransition)]
async fn interval(state: IntervalState, cx: TransitionIn<AutoSend<Bot>>, ans: String,) -> TransitionOut<Dialogue> {
   let info = if ans == "/" {
      String::from("Интервал не изменён")
   } else {
      // Check access rights
      if !state.state.is_admin {
         String::from("Недостаточно прав")
      } else {
         // Checking the correctness of the input
         match ans.parse::<u32>() {
            Ok(v) => {
               // Save to database
               if let Ok(()) = set::set_interval(v as i32 *3600).await {
                  format!("Новый интервал в {} ч. сохранён", ans)
               } else {
                  String::from("Ошибка сохранения интервала, обратитесь к разработчику")
               }
            },
            _ =>  format!("Неверный ввод, ожидалось целое число, например 1 для часового интервала, вы ввели {}. Интервал не изменён", ans),
         }
      }
   };

   cx.answer(info)
   .reply_markup(one_button_markup("В начало"))
   .await?;
   next(StartState { restarted: false })
}
