/* ===============================================================================
Bot to support Telegram channel of 2:5011 Fidonet
Main module, based on teloxide/examples/admin_bot. 12 March 2021.
----------------------------------------------------------------------------
Licensed under the terms of the GPL version 3.
http://www.gnu.org/licenses/gpl-3.0.html
Copyright (c) 2020 by Artem Khomenko _mag12@yahoo.com.
=============================================================================== */

use std::str::FromStr;
use std::{convert::Infallible, env, net::SocketAddr};
use teloxide::{prelude::*, types::ChatPermissions, utils::command::BotCommand};
use teloxide::{dispatching::update_listeners, };
use tokio::sync::mpsc;
use warp::Filter;

use reqwest::StatusCode;

// Derive BotCommand to parse text with a command into this enumeration.
//
//  1. rename = "lowercase" turns all the commands into lowercase letters.
//  2. `description = "..."` specifies a text before all the commands.
//
// That is, you can just call Command::descriptions() to get a description of
// your commands in this format:
// %GENERAL-DESCRIPTION%
// %PREFIX%%COMMAND% - %DESCRIPTION%
#[derive(BotCommand)]
#[command(
   rename = "lowercase",
   description = "Use commands in format /%command% %num% %unit%",
   parse_with = "split"
)]
enum Command {
   #[command(description = "kick user from chat.")]
   Kick,
   #[command(description = "ban user in chat.")]
   Ban {
      time: u32,
      unit: UnitOfTime,
   },
   #[command(description = "mute user in chat.")]
   Mute {
      time: u32,
      unit: UnitOfTime,
   },
   Help,
}

enum UnitOfTime {
   Seconds,
   Minutes,
   Hours,
}

impl FromStr for UnitOfTime {
   type Err = &'static str;
   fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
      match s {
         "h" | "hours" => Ok(UnitOfTime::Hours),
         "m" | "minutes" => Ok(UnitOfTime::Minutes),
         "s" | "seconds" => Ok(UnitOfTime::Seconds),
         _ => Err("Allowed units: h, m, s"),
      }
   }
}

// Calculates time of user restriction.
fn calc_restrict_time(time: u32, unit: UnitOfTime) -> u32 {
   match unit {
      UnitOfTime::Hours => time * 3600,
      UnitOfTime::Minutes => time * 60,
      UnitOfTime::Seconds => time,
   }
}

type Cx = UpdateWithCx<Message>;

// Mute a user with a replied message.
async fn mute_user(cx: &Cx, time: u32) -> ResponseResult<()> {
   match cx.update.reply_to_message() {
      Some(msg1) => {
         cx.bot
               .restrict_chat_member(
                  cx.update.chat_id(),
                  msg1.from().expect("Must be MessageKind::Common").id,
                  ChatPermissions::default(),
               )
               .until_date(cx.update.date + time as i32)
               .send()
               .await?;
      }
      None => {
         cx.reply_to("Use this command in reply to another message").send().await?;
      }
   }
   Ok(())
}

// Kick a user with a replied message.
async fn kick_user(cx: &Cx) -> ResponseResult<()> {
   match cx.update.reply_to_message() {
      Some(mes) => {
         // bot.unban_chat_member can also kicks a user from a group chat.
         cx.bot.unban_chat_member(cx.update.chat_id(), mes.from().unwrap().id).send().await?;
      }
      None => {
         cx.reply_to("Use this command in reply to another message").send().await?;
      }
   }
   Ok(())
}

// Ban a user with replied message.
async fn ban_user(cx: &Cx, time: u32) -> ResponseResult<()> {
   match cx.update.reply_to_message() {
      Some(message) => {
         cx.bot
               .kick_chat_member(
                  cx.update.chat_id(),
                  message.from().expect("Must be MessageKind::Common").id,
               )
               .until_date(cx.update.date + time as i32)
               .send()
               .await?;
      }
      None => {
         cx.reply_to("Use this command in a reply to another message!").send().await?;
      }
   }
   Ok(())
}

async fn action(cx: UpdateWithCx<Message>, command: Command) -> ResponseResult<()> {
   match command {
      Command::Help => cx.answer(Command::descriptions()).send().await.map(|_| ())?,
      Command::Kick => kick_user(&cx).await?,
      Command::Ban { time, unit } => ban_user(&cx, calc_restrict_time(time, unit)).await?,
      Command::Mute { time, unit } => mute_user(&cx, calc_restrict_time(time, unit)).await?,
   };

   Ok(())
}



async fn handle_rejection(error: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
   log::error!("Cannot process the request due to: {:?}", error);
   Ok(StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn webhook<'a>(bot: Bot) -> impl update_listeners::UpdateListener<Infallible> {
   // Heroku auto defines a port value
   let teloxide_token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN env variable missing");
   let port: u16 = env::var("PORT")
       .expect("PORT env variable missing")
       .parse()
       .expect("PORT value to be integer");
   // Heroku host example .: "heroku-ping-pong-bot.herokuapp.com"
   let host = env::var("HOST").expect("have HOST env variable");
   let path = format!("bot{}", teloxide_token);
   let url = format!("https://{}/{}", host, path);

   bot.set_webhook(url).send().await.expect("Cannot setup a webhook");

   let (tx, rx) = mpsc::unbounded_channel();

   let server = warp::post()
      .and(warp::path(path))
      .and(warp::body::json())
      .map(move |json: serde_json::Value| {
         let try_parse = match serde_json::from_str(&json.to_string()) {
            Ok(update) => Ok(update),
            Err(error) => {
                  log::error!(
                     "Cannot parse an update.\nError: {:?}\nValue: {}\n\
                     This is a bug in teloxide, please open an issue here: \
                     https://github.com/teloxide/teloxide/issues.",
                     error,
                     json
                  );
                  Err(error)
            }
         };
         if let Ok(update) = try_parse {
            tx.send(Ok(update)).expect("Cannot send an incoming update from the webhook")
         }

         StatusCode::OK
      })
      .recover(handle_rejection);

   let serve = warp::serve(server);

   let address = format!("0.0.0.0:{}", port);
   tokio::spawn(serve.run(address.parse::<SocketAddr>().unwrap()));
   rx
}

#[tokio::main]
async fn main() {
   run().await;
}

/*async fn run() {
   teloxide::enable_logging!();
   log::info!("Starting admin_bot...");

   let bot = Bot::from_env();

   let bot_name: String = panic!("Your bot's name here");
   teloxide::commands_repl(bot, bot_name, action).await;
}*/

async fn run() {
   teloxide::enable_logging!();
   log::info!("Starting N5011_bot...");

   let bot = Bot::from_env();

   let cloned_bot = bot.clone();
   let bot_name: String = panic!("N5011_bot");
   teloxide::commands_repl_with_listener(
      bot,
      &bot_name,
      action,
      webhook(cloned_bot).await,
   )
   .await;
}