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
   utils::command::BotCommand, dispatching::update_listeners,
   types::{ReplyMarkup, KeyboardButton, ReplyKeyboardMarkup, },
};

use crate::database as db;


#[derive(Transition, From)]
pub enum Dialogue {
   Start(StartState),
   SelectCommand(SelectCommandState),
   StartChangeOrigin(StartChangeOriginState),
   ReceiveOrigin(ReceiveOriginState),
}

impl Default for Dialogue {
   fn default() -> Self {
       Self::Start(StartState)
   }
}

pub struct StartState;

#[teloxide(subtransition)]
async fn start(state: StartState, cx: TransitionIn, _ans: String,) -> TransitionOut<Dialogue> {
   // Extract user id
   let user = cx.update.from();
   if user.is_none() {
      cx.answer_str("Error, no user");
      return next(state);
   }

   // For admin and regular users there is different interface
   let user_id = user.unwrap().id;
   let is_admin = db::is_admin(user_id);

   // Prepare menu
   let commands = if is_admin {
      vec![KeyboardButton::new("/origin"),
      KeyboardButton::new("/List"),
      ]
   } else {
      vec![KeyboardButton::new("Изменить ориджин")]
   };

   let markup = ReplyKeyboardMarkup::default()
   .append_row(commands)
   .resize_keyboard(true);

   cx.answer("Добро пожаловать. Выберите команду на кнопке внизу")
   .reply_markup(ReplyMarkup::ReplyKeyboardMarkup(markup))
   .send()
   .await?;
   next(SelectCommandState { user_id, is_admin })
}

pub struct SelectCommandState {
   user_id: i32,
   is_admin: bool,
}

#[teloxide(subtransition)]
async fn select_command(
   state: SelectCommandState,
   cx: TransitionIn,
   ans: String,
) -> TransitionOut<Dialogue> {
   // Handle commands
   if ans == "Изменить ориджин" {
      // Collect info about update
      let descr = db::user_descr(state.user_id).await;
      let descr = format!("Ваш текущий ориджин {}.\n Пожалуйста, введите строку вида 2:5011/102,Fips_BBS,Ufa,Artem_G.Khomenko или просто /, чтобы оставить текущую информацию без изменений", descr);

      let markup = ReplyKeyboardMarkup::default()
      .append_row(vec![KeyboardButton::new("/")])
      .resize_keyboard(true);

      cx.answer(descr)
      .reply_markup(ReplyMarkup::ReplyKeyboardMarkup(markup))
      .send().
      await?;
      next(state)
   } else {
         cx.answer_str(format!("Неизвестная команда {}", ans)).await?;

         // Stay in previous state
         next(state)
   }
}


pub struct StartChangeOriginState;

#[teloxide(subtransition)]
async fn start_origin(
   _state: StartChangeOriginState,
   cx: TransitionIn,
   _ans: String,
) -> TransitionOut<Dialogue> {
   cx.answer_str("Введите новый ориджин").await?;
   next(ReceiveOriginState)
}

// #[derive(Generic)]
pub struct ReceiveOriginState;

#[teloxide(subtransition)]
async fn receive_origin(
    state: ReceiveOriginState,
    cx: TransitionIn,
    ans: String,
) -> TransitionOut<Dialogue> {
    cx.answer_str("Ваш ориджин сохранён").await?;
    exit()
}