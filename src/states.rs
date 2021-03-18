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
   types::{ReplyMarkup, KeyboardButton, ReplyKeyboardMarkup, },
};
use std::convert::TryFrom;

use crate::database as db;
use crate::settings as set;


// FSM states
#[derive(Transition, From)]
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

// Frequently used start menu
fn markup_for_start() -> ReplyMarkup {
   let markup = ReplyKeyboardMarkup::default()
   .append_row(vec![KeyboardButton::new("В начало")])
   .resize_keyboard(true);
   ReplyMarkup::ReplyKeyboardMarkup(markup)
}

// Frequently used start menu
fn markup_for_cancel() -> ReplyMarkup {
   let markup = ReplyKeyboardMarkup::default()
   .append_row(vec![KeyboardButton::new("/")])
   .resize_keyboard(true);
   ReplyMarkup::ReplyKeyboardMarkup(markup)
}


pub struct StartState {
   restarted: bool,
}

#[teloxide(subtransition)]
async fn start(state: StartState, cx: TransitionIn, _ans: String,) -> TransitionOut<Dialogue> {
   // Extract user id
   let user = cx.update.from();
   if user.is_none() {
      cx.answer_str("Error, no user").await?;
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

   let markup = ReplyKeyboardMarkup::default()
   .append_row(commands)
   .resize_keyboard(true);

   let info = String::from(if state.restarted { "Извините, бот был перезапущен.\n" } else {""});
   let info = info + "Добро пожаловать. Выберите команду на кнопке внизу";

   cx.answer(info)
   .reply_markup(ReplyMarkup::ReplyKeyboardMarkup(markup))
   .send()
   .await?;
   next(CommandState { user_id, is_admin })
}

pub struct CommandState {
   user_id: i32,
   is_admin: bool,
}

#[teloxide(subtransition)]
async fn select_command(state: CommandState, cx: TransitionIn, ans: String,) -> TransitionOut<Dialogue> {
   // Parse text from user
   let command = Command::try_from(ans.as_str());
   if command.is_err() {
      cx.answer_str(format!("Неизвестная команда {}. Пожалуйста, выберите одну из команд внизу (если панель с кнопками скрыта, откройте её)", ans)).await?;

      // Stay in previous state
      return next(state)
   }

   // Handle commands
   match command.unwrap() {
      Command::Origin => {
         // Collect info about update
         let info = db::user_descr(state.user_id).await;
         let info = format!("Ваш текущий ориджин\n{}\nПожалуйста, введите строку вида\n2:5011/102 город, ФИО\n Для отказа нажмите /", info);

         cx.answer(info)
         .reply_markup(markup_for_cancel())
         .send().
         await?;

         next(OriginState { state })
      }

      Command::Interval => {
         let info = format!("Время с момента последнего сообщения пользователя для напоминания его адреса {} сек. Введите новый интервал в секундах или / для отмены", set::interval());

         cx.answer(info)
         .reply_markup(markup_for_cancel())
         .send().
         await?;

         next(IntervalState { state })
      }
      _ => next(state),
   }
}

// #[derive(Generic)]
pub struct OriginState {
   state: CommandState,
}

#[teloxide(subtransition)]
async fn origin(state: OriginState, cx: TransitionIn, ans: String,) -> TransitionOut<Dialogue> {
   let info = if ans == "/" {
      String::from("Ориджин не изменён")
   } else {
      // Save to database
      db::update_user_descr(state.state.user_id, &ans).await;

      format!("Ваш новый ориджин {} сохранён", ans)
   };

   cx.answer(info)
   .reply_markup(markup_for_start())
   .send().
   await?;
   next(StartState { restarted: false })
}

// #[derive(Generic)]
pub struct IntervalState {
   state: CommandState,
}

#[teloxide(subtransition)]
async fn interval(state: IntervalState, cx: TransitionIn, ans: String,) -> TransitionOut<Dialogue> {
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
               if let Ok(()) = set::set_interval(v as i32).await {
                  format!("Новый интервал в {} секунд сохранён", ans)
               } else {
                  String::from("Ошибка сохранения интервала, обратитесь к разработчику")
               }
            },
            _ =>  format!("Неверный ввод, ожидалось целое число, например 3600 для часового интервала, вы ввели {}. Интервал не изменён", ans),
         }
      }
   };

   cx.answer(info)
   .reply_markup(markup_for_start())
   .send().
   await?;
   next(StartState { restarted: false })
}
