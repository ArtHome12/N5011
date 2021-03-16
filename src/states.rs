/* ===============================================================================
Bot to support Telegram channel of 2:5011 Fidonet
Dialogue FSM. 16 March 2021.
----------------------------------------------------------------------------
Licensed under the terms of the GPL version 3.
http://www.gnu.org/licenses/gpl-3.0.html
Copyright (c) 2020 by Artem Khomenko _mag12@yahoo.com.
=============================================================================== */

use derive_more::From;
use teloxide_macros::Transition;
use teloxide::prelude::*;
use teloxide_macros::teloxide;

#[derive(Transition, From)]
pub enum Dialogue {
   Start(StartState),
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
async fn start(
    _state: StartState,
    cx: TransitionIn,
    _ans: String,
) -> TransitionOut<Dialogue> {
    cx.answer_str("Добро пожаловать. Выберите команду на кнопке внизу").await?;
    exit()
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