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
use native_tls::{TlsConnector};
use postgres_native_tls::MakeTlsConnector;

mod database;
use database::{self as db, };

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
   // List,
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

async fn run() {
   teloxide::enable_logging!();
   log::info!("Starting N5011_bot...");

   let database_url = env::var("DATABASE_URL").expect("DATABASE_URL env variable missing");
   log::info!("{}", database_url);

   let connector = TlsConnector::builder()
   .danger_accept_invalid_certs(true)
   .build()
   .unwrap();
   let connector = MakeTlsConnector::new(connector);

   // Откроем БД
   let (client, connection) =
      tokio_postgres::connect(&database_url, connector).await
         .expect("Cannot connect to database");

   // The connection object performs the actual communication with the database,
   // so spawn it off to run on its own.
   tokio::spawn(async move {
      if let Err(e) = connection.await {
         log::info!("Database connection error: {}", e);
      }
   });

   // Сохраним доступ к БД
   match db::DB.set(client) {
      Ok(_) => log::info!("Database connected"),
      _ => log::info!("Something wrong with database"),
   }

   // Создадим таблицу в БД, если её ещё нет
   db::check_database().await;

   let bot = Bot::from_env();

   Dispatcher::new(bot.clone())
   .messages_handler(|rx: DispatcherHandlerRx<Message>| {
      rx.for_each_concurrent(None, |message| async move {
         handle_message(message).await.expect("Something wrong with the bot!");
      })
   })
   /* .callback_queries_handler(|rx: DispatcherHandlerRx<CallbackQuery>| {
      rx.for_each_concurrent(None, |cx| async move {
         handle_callback(cx).await
      })
   }) */
   .dispatch_with_listener(
      webhook(bot).await,
      LoggingErrorHandler::with_custom_text("An error from the update listener"),
   )
   .await;
}

async fn handle_message(cx: UpdateWithCx<Message>) -> ResponseResult<Message> {

   // Для различения, в личку или в группу пишут
   let chat_id = cx.update.chat_id();

   // Обрабатываем сообщение, только если оно пришло в личку
   if chat_id < 0 {
      return Ok(cx.update);
   }

   match cx.update.text() {
      None => cx.answer_str("Текстовое сообщение, пожалуйста!").await,
      Some(text) => {
         // Попробуем получить команду
         if let Ok(command) = Command::parse(text, "n5011_bot") {
            let cx_update = cx.update.clone();
            action(cx, command)
            .await
            .map(|_| Ok(cx_update))?
         } else {
            // Regular message
            if let Some(user) = cx.update.from() {
               // Collect info about update
               let user_id = user.id;
               let def_descr = user.username.clone().unwrap_or_default();
               let def_descr = user.full_name() + &def_descr;
               let time = cx.update.date;
               
               // Make announcement if needs
               match db::announcement(user_id, time, &def_descr).await {
                  Some(announcement) => {
                     cx.reply_to(announcement)
                     .send()
                     .await
                  }

                  // No needs announce
                  _ => Ok(cx.update),
               }
            } else {
               log::info!("Error no user in cx.update.from()");
               Ok(cx.update)
            }
         }
      }
   }
}
