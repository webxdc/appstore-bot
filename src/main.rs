#![warn(clippy::all, clippy::indexing_slicing, clippy::unwrap_used)]
mod bot;
mod bot_commands;
mod cli;
mod db;
mod messages;
mod request_handlers;
mod utils;

use anyhow::Context;
use bot::Bot;
use clap::Parser;
use cli::{BotActions, BotCli};
use log::info;
use surrealdb::sql::Id;
use surrealdb::sql::Thing;
use tokio::fs;
use tokio::signal;
use utils::get_db_path;

use crate::db::DB;
use crate::request_handlers::AppInfo;

const DB_PATH: &str = "bot.db";
const GENESIS_QR: &str = "genesis_invite_qr.png";
const INVITE_QR: &str = "1o1_invite_qr.png";
const SHOP_NAME: &str = "appstore.xdc";
const SUBMIT_HELPER: &str = "review_helper.xdc";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cli = BotCli::parse();

    match &cli.action {
        BotActions::Import => {
            info!("Importing webxdcs from 'import/'");
            let db = DB::new(&get_db_path()?).await?;
            let files: Vec<_> = std::fs::read_dir("import/")?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|e| e.is_file())
                .collect();

            if files.is_empty() {
                println!("No xdcs to add in ./import")
            }

            for file in &files {
                if file
                    .file_name()
                    .and_then(|a| a.to_str())
                    .context("Can't get filename for imported file")?
                    .ends_with(".xdc")
                {
                    let mut app_info = AppInfo::from_xdc(file).await?;
                    app_info.active = true;
                    app_info.author_name = "Appstore bot".to_string();
                    app_info.author_email = "appstorebot@testrun.org".to_string();

                    let missing = app_info.generate_missing_list();
                    if missing.is_empty() {
                        let mut new_path = file
                            .parent()
                            .and_then(|a| a.parent())
                            .context("path could not be constructed")?
                            .to_path_buf();

                        new_path.push("xdcs");
                        new_path.push(file.file_name().context("Direntry has no filename")?);
                        fs::rename(file, &new_path).await?;
                        app_info.xdc_blob_dir = Some(new_path);

                        db.create_app_info(
                            &app_info,
                            Thing {
                                tb: "app_info".to_string(),
                                id: Id::rand(),
                            },
                        )
                        .await?;
                        println!("Added {} to apps", app_info.name);
                    } else {
                        println!(
                            "The app {} is missing some data: {:?}",
                            file.as_os_str().to_str().context("Can't convert to str")?,
                            missing
                        )
                    }
                }
            }
        }
        BotActions::ShowQr => {
            let db = DB::new(&get_db_path()?).await?;
            match db.get_config().await? {
                Some(config) => {
                    println!("You can find png files of the qr codes at bots home dir");
                    println!("Genisis invite qr:");
                    qr2term::print_qr(config.genesis_qr)?;
                    println!("Bot invite qr:");
                    qr2term::print_qr(config.invite_qr)?;
                }
                None => println!("Bot not configured yet, start the bot first."),
            }
        }
        BotActions::Start => {
            let mut bot = Bot::new().await?;
            bot.start().await;
            signal::ctrl_c().await?;
        }
    }
    Ok(())
}
