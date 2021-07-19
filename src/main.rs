/* ===============================================================================
Bot to support Telegram channel of 2:5011 Fidonet
Main module, based on teloxide/examples/admin_bot. 12 March 2021.
----------------------------------------------------------------------------
Licensed under the terms of the GPL version 3.
http://www.gnu.org/licenses/gpl-3.0.html
Copyright (c) 2020 by Artem Khomenko _mag12@yahoo.com.
=============================================================================== */

use std::{convert::Infallible, env, net::SocketAddr};
use teloxide::{
   prelude::*, types::ChatPermissions,
   dispatching::{update_listeners::{self, StatefulListener}, stop_token::AsyncStopToken}
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::Filter;
use reqwest::{StatusCode, Url};
use native_tls::{TlsConnector};
use postgres_native_tls::MakeTlsConnector;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};

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

pub async fn webhook<'a>(bot: AutoSend<Bot>) -> impl update_listeners::UpdateListener<Infallible> {
   // Heroku auto defines a port value
   let teloxide_token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN env variable missing");
   let port: u16 = env::var("PORT")
       .expect("PORT env variable missing")
       .parse()
       .expect("PORT value to be integer");
   // Heroku host example .: "heroku-ping-pong-bot.herokuapp.com"
   let host = env::var("HOST").expect("have HOST env variable");
   let path = format!("bot{}", teloxide_token);
   let url =  Url::parse(&format!("https://{}/{}", host, path))
   .unwrap();

   bot.set_webhook(url).await.expect("Cannot setup a webhook");

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

   let (stop_token, stop_flag) = AsyncStopToken::new_pair();

   let addr = format!("0.0.0.0:{}", port).parse::<SocketAddr>().unwrap();
   let server = warp::serve(server);
   let (_addr, fut) = server.bind_with_graceful_shutdown(addr, stop_flag);

   // You might want to use serve.key_path/serve.cert_path methods here to
   // setup a self-signed TLS certificate.

   tokio::spawn(fut);
   let stream = UnboundedReceiverStream::new(rx);

   fn streamf<S, T>(state: &mut (S, T)) -> &mut S { &mut state.0 }

   StatefulListener::new((stream, stop_token), streamf, |state: &mut (_, AsyncStopToken)| state.1.clone())
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
   let admin1  = env::var("ADMIN_ID1").expect("ADMIN_ID1 env variable missing").parse().unwrap_or_default();
   let admin2 = env::var("ADMIN_ID2").expect("ADMIN_ID2 env variable missing").parse().unwrap_or_default();
   set::set_admins(admin1, admin2).expect("ADMIN_ID2 set fail");

   let bot = Bot::from_env().auto_send();

   teloxide::dialogues_repl_with_listener(
      bot.clone(),
      |message, dialogue| async move {
         handle_message(message, dialogue).await.expect("Something wrong with the bot!")
      },
      webhook(bot).await
   )
  .await;
}

async fn handle_message(cx: UpdateWithCx<AutoSend<Bot>, Message>, dialogue: Dialogue) -> TransitionOut<Dialogue> {

   let user = cx.update.from();
   if user.is_none() {
      log::info!("Error no user in cx.update.from()");
      return next(dialogue);
   }

   // Collect info about update
   let user = user.unwrap();
   let user_id = user.id;
   let time = cx.update.date;
   let text = String::from(cx.update.text().unwrap_or_default());

   // Collect information and guaranteed to save the user in the database
   let announcement = db::announcement(user_id, time).await;

   // Negative for chats, positive personal
   let chat_id = cx.update.chat_id();

   if chat_id > 0 {
      if text == "" {
         if let Err(e) = cx.answer("Текстовое сообщение, пожалуйста!").await {
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
      if text == "[+]" && msg.is_some() && is_admin(&cx.requester, chat_id, user_id).await {

         // Extract the author and restrict
         if let Some(from) = msg.unwrap().from() {
            let res = cx.requester
            .restrict_chat_member(
                chat_id,
                from.id,
                ChatPermissions::default(),
            )
            // .until_date(cx.update.date as i64 + 3600i64)
            .until_date(
               DateTime::<Utc>::from_utc(
                   NaiveDateTime::from_timestamp(cx.update.date as i64, 0),
                   Utc,
               ) + Duration::hours(1)
            )
            .await;

            // Notify chat members
            let res = if let Err(e) = res {
               cx.reply_to(format!("{}", e))
            } else {
               let name = from.username.clone().unwrap_or_default();
               let text = format!("RO на часок. Не расстраивайся, {}!", name);
               cx.requester.send_message(chat_id, text)
            };
            if let Err(e) = res.await {
               log::info!("Error main handle_message 2 (): {}", e);
            }
         }
      }

      // Make announcement in chat if needs
      if let Some(announcement) = announcement {
         if let Err(e) = cx.reply_to(announcement).await {
            log::info!("Error main handle_message 3 (): {}", e);
         }
      }

      next(dialogue)
   }
}


async fn is_admin(bot: & AutoSend<Bot>, chat_id: i64, user_id: i64) -> bool {
   let member = bot.get_chat_member(chat_id, user_id)
   .send()
   .await;

   set::is_admin(user_id) || (
      member.is_ok()
      && member.unwrap().kind.can_restrict_members()
   )
}