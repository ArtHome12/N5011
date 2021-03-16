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
async fn start(_state: StartState, cx: TransitionIn, _ans: String,) -> TransitionOut<Dialogue> {

   // For admin and regular users there is different interface
   let user = cx.update.from();
   let is_admin = if user.is_some() {db::is_admin(user.unwrap().id)} else {false};

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

   cx.reply_to("Добро пожаловать. Выберите команду на кнопке внизу")
   .reply_markup(ReplyMarkup::ReplyKeyboardMarkup(markup))
   .send()
   .await?;
   exit()
}

pub struct SelectCommandState;

#[teloxide(subtransition)]
async fn select_command(
   _state: SelectCommandState,
   cx: TransitionIn,
   ans: String,
) -> TransitionOut<Dialogue> {
   cx.answer_str(format!("Selected {}", ans)).await?;
   next(StartState)
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