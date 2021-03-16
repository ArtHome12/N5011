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
use tokio::sync::mpsc;
use warp::Filter;
use reqwest::StatusCode;
use native_tls::{TlsConnector};
use postgres_native_tls::MakeTlsConnector;

extern crate frunk;

use crate::states::Dialogue;


mod states;
mod database;
use database::{self as db, };


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
   let admin: i32 = env::var("ADMIN_ID1").expect("ADMIN_ID1 env variable missing").parse().unwrap_or_default();
   db::ADMIN_1.set(admin).expect("ADMIN_ID1 set fail");
   let admin:i32 = env::var("ADMIN_ID2").expect("ADMIN_ID2 env variable missing").parse().unwrap_or_default();
   db::ADMIN_2.set(admin).expect("ADMIN_ID2 set fail");

   let bot = Bot::from_env();

   /* Dispatcher::new(bot.clone())
   .messages_handler(DialogueDispatcher::new(|cx| async move {
      let res = handle_message(cx).await;
      if let Err(e) = res {
         log::info!("run error {}", e);
         DialogueStage::Exit
      } else {
         res.unwrap()
      }
   })) */

   /*.messages_handler(|rx: DispatcherHandlerRx<Message>| {
      rx.for_each_concurrent(None, |message| async move {
         handle_message(message).await.expect("Something wrong with the bot!");
      })
   })*/
   /* .callback_queries_handler(|rx: DispatcherHandlerRx<CallbackQuery>| {
      rx.for_each_concurrent(None, |cx| async move {
         handle_callback(cx).await
      })
   }) */
   /* .dispatch_with_listener(
      webhook(bot).await,
      LoggingErrorHandler::with_custom_text("An error from the update listener"),
   )*/

   teloxide::dialogues_repl_with_listener(
      bot.clone(),
      |message, dialogue| async move {
         handle_message(message, dialogue).await.expect("Something wrong with the bot!")
      },
      webhook(bot).await
   )
  .await;

   /*let handler = Arc::new(handler);

   Dispatcher::new(bot)
      .messages_handler(DialogueDispatcher::new(
          move |DialogueWithCx { cx, dialogue }: DialogueWithCx<Message, D, Infallible>| {
              let handler = Arc::clone(&handler);

              async move {
                  let dialogue = dialogue.expect("std::convert::Infallible");
                  handler(cx, dialogue).await
              }
          },
      ))
      .dispatch_with_listener(
         webhook(bot).await,
          LoggingErrorHandler::with_custom_text("An error from the update listener"),
      )
      .await;*/

}

async fn handle_message(cx: UpdateWithCx<Message>, dialogue: Dialogue) -> TransitionOut<Dialogue> {

   // Collect information and guaranteed to save the user in the database
   let announcement = if let Some(user) = cx.update.from() {
      // Collect info about update
      let user_id = user.id;
      let def_descr = user.username.clone().unwrap_or_default();
      let def_descr = if def_descr.len() > 0 {String::from(" @") + &def_descr} else {String::default()};
      let def_descr = user.full_name() + &def_descr;
      let time = cx.update.date;

      // Make announcement if needs
      db::announcement(user_id, time, &def_descr).await
   } else {
      log::info!("Error no user in cx.update.from()");
      None
   };

   // Negative for chats, positive personal
   let chat_id = cx.update.chat_id();

   if chat_id > 0 {
      // Private messages with FSM
      match cx.update.text_owned() {
         None => {
            cx.answer_str("Текстовое сообщение, пожалуйста!").await?;
            next(dialogue)
         }
         Some(ans) => dialogue.react(cx, ans).await,
      }
   } else {
      // Make announcement in chat if needs
      if let Some(announcement) = announcement {
         cx.reply_to(announcement)
         .send()
         .await?;
      }

      next(dialogue)
   }
}
