//! Entry for the bot code
use anyhow::{Context as _, Result};
use deltachat::{
    chat::ChatId,
    config::Config,
    context::Context,
    message::{Message, MsgId},
    stock_str::StockStrings,
    EventType, Events,
};
use log::{debug, error, info, trace, warn};
use std::{env, path::PathBuf, sync::Arc};
use tokio::fs;

use crate::{
    db::DB,
    request_handlers::{shop, AppInfo, Chat, ChatType},
    utils::configure_from_env,
};

/// Github Bot state
pub struct State {
    pub db: DB,
}

impl State {
    pub fn get_apps(&self) -> Vec<AppInfo> {
        vec![
            AppInfo {
                name: "App 3".to_string(),
                author_name: "Author 3".to_string(),
                author_email: "author1@example.com".to_string(),
                source_code_url: "https://github.com/author1/app3".to_string(),
                description: "This is a description for App 3.".to_string(),
                xdc_blob_url: "https://blobstore.com/app3".to_string(),
                version: "1.0.0".to_string(),
                image: "https://via.placeholder.com/640".to_string(),
            },
            AppInfo {
                name: "App 2".to_string(),
                author_name: "Author 2".to_string(),
                author_email: "author2@example.com".to_string(),
                source_code_url: "https://github.com/author2/app2".to_string(),
                description: "This is a description for App 2.".to_string(),
                xdc_blob_url: "https://blobstore.com/app2".to_string(),
                version: "2.0.0".to_string(),
                image: "https://via.placeholder.com/640".to_string(),
            },
        ]
    }
}

/// Github Bot
pub struct Bot {
    dc_ctx: Context,
    state: Arc<State>,
}

impl Bot {
    pub async fn new() -> Self {
        let dbdir = env::current_dir().unwrap().join("deltachat.db");

        std::fs::create_dir_all(dbdir.clone())
            .context("failed to create db folder")
            .unwrap();

        let dbfile = dbdir.join("db.sqlite");
        let ctx = Context::new(dbfile.as_path(), 1, Events::new(), StockStrings::new())
            .await
            .context("Failed to create context")
            .unwrap();

        if !ctx.get_config_bool(Config::Configured).await.unwrap() {
            info!("start configuring...");
            configure_from_env(&ctx).await.unwrap();
            info!("configuration done");
        }

        if !PathBuf::from("appstore_manifest.json").exists() {
            fs::write("appstore_manifest.json", "[]").await.unwrap();
        }

        if !PathBuf::from("xdcs").exists() {
            fs::create_dir("xdcs").await.unwrap();
        }

        let db = DB::new("bot.db").await;

        Self {
            dc_ctx: ctx,
            state: Arc::new(State { db }),
        }
    }

    /// Start the bot which includes:
    /// - starting dc-message-receive loop
    /// - starting webhook-receive loop
    ///   - starting receiving server
    pub async fn start(&mut self) {
        // start dc message handler
        let events_emitter = self.dc_ctx.get_event_emitter();
        let ctx = self.dc_ctx.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            while let Some(event) = events_emitter.recv().await {
                if let Err(e) = Self::dc_event_handler(&ctx, state.clone(), event.typ).await {
                    warn!("{}", e)
                }
            }
        });

        info!("initiated dc message handler (1/2)");

        self.dc_ctx.start_io().await;

        info!("initiated dc io (2/2)");

        info!("successfully started bot! 🥳");
    }

    /// Handle _all_ dc-events
    async fn dc_event_handler(
        context: &Context,
        state: Arc<State>,
        event: EventType,
    ) -> anyhow::Result<()> {
        match event {
            EventType::ConfigureProgress { progress, comment } => {
                trace!("DC: Configuring progress: {progress} {comment:?}")
            }
            EventType::Info(msg) => trace!("DC: {msg}"),
            EventType::Warning(msg) => warn!("DC: {msg}"),
            EventType::Error(msg) => error!("DC: {msg}"),
            EventType::ConnectivityChanged => trace!("DC: ConnectivityChanged"),
            EventType::IncomingMsg { chat_id, msg_id } => {
                Self::handle_dc_message(context, state, chat_id, msg_id).await?
            }
            EventType::WebxdcStatusUpdate {
                msg_id,
                status_update_serial,
            } => {
                let update_string = context
                    .get_status_update(msg_id, status_update_serial)
                    .await?;

                Self::handle_dc_webxdc_update(context, state, msg_id, update_string).await?
            }
            other => {
                debug!("DC: [unhandled event] {other:?}");
            }
        }
        Ok(())
    }

    /// Handles chat messages from clients
    async fn handle_dc_message(
        context: &Context,
        state: Arc<State>,
        chat_id: ChatId,
        _msg_id: MsgId,
    ) -> Result<()> {
        if let Ok(Some(chat)) = state.db.get_chat(chat_id).await {
            match chat.chat_type {
                ChatType::Release => todo!(),
                ChatType::Shop => shop::handle_message(context, chat_id).await?,
            }
        } else {
            let chat = Chat {
                chat_type: ChatType::Shop,
                chat_id: chat_id,
                publisher: None,
                tester: Vec::new(),
                creator: None,
            };

            //TODO: test for single chat

            state.db.create_chat(chat).await?;
            shop::handle_message(context, chat_id).await?;
        }
        Ok(())
    }

    /// Handles chat messages from clients
    async fn handle_dc_webxdc_update(
        context: &Context,
        state: Arc<State>,
        msg_id: MsgId,
        update: String,
    ) -> anyhow::Result<()> {
        let msg = Message::load_from_db(context, msg_id).await?;
        let chat_id = msg.get_chat_id();
        let chat: Chat = state
            .db
            .get_chat(chat_id)
            .await?
            .ok_or(anyhow::anyhow!("No chat for this message"))?;

        match chat.chat_type {
            ChatType::Release => todo!(),
            ChatType::Shop => {
                shop::handle_status_update(context, state, chat_id, msg_id, update).await?
            }
        }

        Ok(())
    }
}
