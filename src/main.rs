/* ===============================================================================
Bot to support Telegram channel of 2:5011 Fidonet
Main module, based on teloxide/examples/admin_bot. 12 March 2021.
----------------------------------------------------------------------------
Licensed under the terms of the GPL version 3.
http://www.gnu.org/licenses/gpl-3.0.html
Copyright (c) 2020 by Artem Khomenko _mag12@yahoo.com.
=============================================================================== */

use std::{convert::Infallible, env, net::SocketAddr};
use teloxide::{prelude::*, dispatching::update_listeners, };
use teloxide::types::ChatPermissions;
use tokio::sync::mpsc;
use warp::Filter;
use reqwest::StatusCode;
use native_tls::{TlsConnector};
use postgres_native_tls::MakeTlsConnector;

// extern crate frunk;

use crate::states::Dialogue;


mod states;
mod database;
mod settings;
use database::{self as db, };
use settings::{self as set, };


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

   // Сохраним коды админов
   let admin1: i32 = env::var("ADMIN_ID1").expect("ADMIN_ID1 env variable missing").parse().unwrap_or_default();
   let admin2:i32 = env::var("ADMIN_ID2").expect("ADMIN_ID2 env variable missing").parse().unwrap_or_default();
   set::set_admins(admin1, admin2).expect("ADMIN_ID2 set fail");

   let bot = Bot::from_env();

   teloxide::dialogues_repl_with_listener(
      bot.clone(),
      |message, dialogue| async move {
         handle_message(message, dialogue).await.expect("Something wrong with the bot!")
      },
      webhook(bot).await
   )
  .await;
}

async fn handle_message(cx: UpdateWithCx<Message>, dialogue: Dialogue) -> TransitionOut<Dialogue> {

   let user = cx.update.from();
   if user.is_none() {
      log::info!("Error no user in cx.update.from()");
      return next(dialogue);
   }

   // Collect info about update
   let user = user.unwrap();
   let user_id = user.id;
   let def_descr = user.username.clone().unwrap_or_default();
   let def_descr = if def_descr.len() > 0 {String::from(" @") + &def_descr} else {String::default()};
   let def_descr = user.full_name() + &def_descr;
   let time = cx.update.date;
   let text = cx.update.text_owned().unwrap_or_default();

   // Collect information and guaranteed to save the user in the database
   let announcement = db::announcement(user_id, time, &def_descr).await;

   // Negative for chats, positive personal
   let chat_id = cx.update.chat_id();

   if chat_id > 0 {
      if text == "" {
         if let Err(e) = cx.answer_str("Текстовое сообщение, пожалуйста!").await {
            log::info!("Error main handle_message(): {}", e);
         }
         next(dialogue)
      } else {
         // Private messages with FSM
         dialogue.react(cx, text).await
      }
   } else {
      // Check moderate command
      let msg = cx.update.reply_to_message();
      if set::is_admin(user_id) && text == "+" && msg.is_some() {

         // Extract the author and restrict
         if let Some(from) = msg.unwrap().from() {
            let res = cx.bot
            .restrict_chat_member(
                chat_id,
                from.id,
                ChatPermissions::default(),
            )
            .until_date(cx.update.date + 3600)
            .send().
            await;

            // Notify chat members
            let res = if let Err(e) = res {
               cx.reply_to(format!("{}", e))
            } else {
               let text = format!("RO на часок. Не расстаивайся, {}!", def_descr);
               cx.bot.send_message(chat_id, text)
            };
            if let Err(e) = res.send().await {
               log::info!("Error main handle_message 2 (): {}", e);
            }
      }
      }

      // Make announcement in chat if needs
      if let Some(announcement) = announcement {
         cx.reply_to(announcement)
         .send()
         .await?;
      }

      next(dialogue)
   }
}
