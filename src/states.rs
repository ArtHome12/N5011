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
   StartChangeOrigin(StartChangeOriginState),
   ReceiveOrigin(ReceiveOriginState),
}

impl Default for Dialogue {
   fn default() -> Self {
       Self::StartChangeOrigin(StartChangeOriginState)
   }
}

pub struct StartChangeOriginState;

#[teloxide(subtransition)]
async fn start(
    _state: StartChangeOriginState,
    cx: TransitionIn,
    _ans: String,
) -> TransitionOut<Dialogue> {
    cx.answer_str("Let's start! What's your full name?").await?;
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
    cx.answer_str("How old are you?").await?;
    exit()
}