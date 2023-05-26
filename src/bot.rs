//! Entry for the bot code
use anyhow::{Context as _, Result};
use deltachat::{
    chat::{self, ChatId, ProtectionStatus},
    config::Config,
    context::Context,
    message::{Message, MsgId, Viewtype},
    securejoin,
    stock_str::StockStrings,
    EventType, Events,
};
use log::{debug, error, info, trace, warn};
use qrcode_generator::QrCodeEcc;
use serde::{Deserialize, Serialize};
use std::{env, sync::Arc};

use crate::{
    db::DB,
    messages::appstore_message,
    request_handlers::{genisis, review, shop, submit, AppInfoId, ChatType},
    utils::{configure_from_env, send_webxdc},
};

#[derive(Serialize, Deserialize)]
pub struct BotConfig {
    pub genesis_qr: String,
    pub invite_qr: String,
    pub tester_group: ChatId,
    pub reviewee_group: ChatId,
    pub genesis_group: ChatId,
}

/// Github Bot state
pub struct State {
    pub db: DB,
    pub config: BotConfig,
}

impl State {
    pub async fn get_apps(&self) -> anyhow::Result<Vec<AppInfoId>> {
        Ok(self.db.get_active_app_infos().await?)
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
        let context = Context::new(dbfile.as_path(), 1, Events::new(), StockStrings::new())
            .await
            .context("Failed to create context")
            .unwrap();

        if !context.get_config_bool(Config::Configured).await.unwrap() {
            info!("start configuring...");
            configure_from_env(&context).await.unwrap();
            info!("configuration done");
        }

        let db_path = env::current_dir()
            .unwrap()
            .join("bot.db")
            .to_str()
            .unwrap()
            .to_string();
        let db = DB::new(&db_path).await;

        let config = match db.get_config().await.unwrap() {
            Some(config) => config,
            None => {
                info!("No configuration found, start configuring...");
                let config = Self::setup(&context).await.unwrap();

                // save config
                db.set_config(&config).await.unwrap();

                // set chat types
                db.set_chat_type(config.genesis_group, ChatType::Genesis)
                    .await
                    .unwrap();
                db.set_chat_type(config.reviewee_group, ChatType::ReviewPool)
                    .await
                    .unwrap();
                db.set_chat_type(config.tester_group, ChatType::TesterPool)
                    .await
                    .unwrap();

                // save qr codes to disk
                qrcode_generator::to_png_to_file(
                    &config.genesis_qr,
                    QrCodeEcc::Low,
                    1024,
                    "genenis_join_qr.png",
                )
                .unwrap();
                println!("Generated genisis group join QR-code at ./genenis_join_qr.png");

                qrcode_generator::to_png_to_file(
                    &config.invite_qr,
                    QrCodeEcc::Low,
                    1024,
                    "1o1_invite_qr.png",
                )
                .unwrap();
                println!("Generated 1:1 invite QR-code at ./1o1_invite_qr.png");

                config
            }
        };

        Self {
            dc_ctx: context,
            state: Arc::new(State { db, config }),
        }
    }

    async fn setup(context: &Context) -> Result<BotConfig> {
        let genesis_group =
            chat::create_group_chat(context, ProtectionStatus::Protected, "Appstore: Genesis")
                .await?;

        let reviewee_group =
            chat::create_group_chat(context, ProtectionStatus::Protected, "Appstore: Publishers")
                .await?;

        let tester_group =
            chat::create_group_chat(context, ProtectionStatus::Protected, "Appstore: Testers")
                .await?;

        let genesis_qr = securejoin::get_securejoin_qr(context, Some(genesis_group))
            .await
            .unwrap();

        let invite_qr = securejoin::get_securejoin_qr(context, None).await.unwrap();

        Ok(BotConfig {
            genesis_qr,
            invite_qr,
            reviewee_group,
            genesis_group,
            tester_group,
        })
    }

    /// Start the bot which includes:
    /// - starting dc-message-receive loop
    pub async fn start(&mut self) {
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
        self.dc_ctx.start_io().await;

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
            EventType::ChatModified(chat_id) => match state.db.get_chat_type(chat_id).await? {
                Some(chat_type) => {
                    let contacts = chat::get_chat_contacts(context, chat_id).await?;
                    let filtered = contacts.into_iter().filter(|ci| !ci.is_special());
                    info!("updating contacts for chat {chat_id}");
                    match chat_type {
                        ChatType::Genesis => {
                            state
                                .db
                                .set_genesis_contacts(&filtered.collect::<Vec<_>>())
                                .await?;
                        }
                        ChatType::ReviewPool => {
                            state
                                .db
                                .set_tester_contacts(&filtered.collect::<Vec<_>>())
                                .await?;
                        }
                        ChatType::TesterPool => {
                            state
                                .db
                                .set_publisher_contacts(&filtered.collect::<Vec<_>>())
                                .await?;
                        }
                        // TODO: handle membership changes in testing and submit group
                        _ => (),
                    };
                }
                None => {
                    info!("Chat {chat_id} is not in the database, adding it as chat with type shop");
                    state.db.set_chat_type(chat_id, ChatType::Shop).await?;
                    chat::send_text_msg(context, chat_id, appstore_message().to_string()).await?;
                    send_webxdc(context, chat_id, "./appstore.xdc").await?;
                }
            },
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
        msg_id: MsgId,
    ) -> Result<()> {
        match state.db.get_chat_type(chat_id).await {
            Ok(Some(chat_type)) => {
                info!("Handling message with type <{chat_type:?}>");
                match chat_type {
                    ChatType::Submit => {
                        let msg = Message::load_from_db(context, msg_id).await?;
                        if msg.get_viewtype() == Viewtype::Webxdc {
                            submit::handle_webxdc(context, chat_id, state, msg).await?;
                        } else {
                            submit::handle_message(context, chat_id, state, msg).await?;
                        }
                    }
                    ChatType::Shop => {
                        let msg = Message::load_from_db(context, msg_id).await?;
                        if msg.get_viewtype() == Viewtype::Webxdc {
                            shop::handle_webxdc(context, msg).await?;
                        } else {
                            shop::handle_message(context, chat_id).await?;
                        }
                    }
                    ChatType::Genesis => {
                        genisis::handle_message(context, state, chat_id, msg_id).await?
                    }
                    ChatType::ReviewPool | ChatType::TesterPool => (),
                    ChatType::Review => {
                        review::handle_message(context, chat_id, state, msg_id).await?;
                    }
                }
            }
            Ok(None) => {
                info!("creating new 1:1 chat with type Shop");
                state.db.set_chat_type(chat_id, ChatType::Shop).await?;
                shop::handle_message(context, chat_id).await?;
            }
            Err(e) => {
                warn!("got some error: {}", e);
            }
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
        let chat_type = state
            .db
            .get_chat_type(chat_id)
            .await?
            .ok_or(anyhow::anyhow!("No chat for this message"))?;

        match chat_type {
            ChatType::Submit => {
                submit::handle_status_update(context, state, chat_id, msg_id, update).await?
            }
            ChatType::Shop => {
                shop::handle_status_update(context, state, chat_id, msg_id, update).await?
            }
            ChatType::ReviewPool | ChatType::TesterPool | ChatType::Genesis => (),
            ChatType::Review => todo!(),
        }

        Ok(())
    }
}
