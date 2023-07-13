//! Utility functions

use std::fs::File;
use std::io::Write;

use anyhow::{bail, Context as _, Result};
use async_zip::tokio::read::fs::ZipFileReader;
use deltachat::{
    chat::{self, ChatId},
    config::Config,
    context::Context,
    message::{Message, MsgId, Viewtype},
};
use directories::ProjectDirs;
use futures::future::join_all;
use serde::Serialize;
use sqlx::{SqliteConnection, Type};
use std::{
    env,
    path::{Path, PathBuf},
};
use tokio::task::JoinHandle;

use crate::{
    bot::State,
    db,
    request_handlers::{AppInfo, WebxdcStatusUpdatePayload, WexbdcManifest},
    STORE_XDC,
};

pub(crate) fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "XDC Store").context("cannot determine home directory")
}

pub async fn configure_from_env(ctx: &Context) -> Result<()> {
    let addr = env::var("addr").context("Missing environment variable addr")?;
    ctx.set_config(Config::Addr, Some(&addr)).await?;
    let pw = env::var("mail_pw").context("Missing environment variable mail_pw")?;
    ctx.set_config(Config::MailPw, Some(&pw)).await?;
    ctx.set_config(Config::Bot, Some("1")).await?;
    ctx.set_config(Config::E2eeEnabled, Some("1")).await?;
    ctx.configure()
        .await
        .context("configure failed, you might have wrong credentials")?;
    Ok(())
}

pub(crate) fn unpack_assets() -> Result<()> {
    let store_bytes = include_bytes!("../assets/store.xdc");

    std::fs::create_dir_all(project_dirs()?.config_dir())?;

    let store_path = Webxdc::Store.get_path().context("cannot get webxdc path")?;
    let mut file = File::create(&store_path)
        .with_context(|| format!("failed to create {}", store_path.display()))?;
    file.write_all(store_bytes)?;
    Ok(())
}

/// Send a webxdc to a chat.
pub async fn send_webxdc(
    context: &Context,
    state: &State,
    chat_id: ChatId,
    webxdc: Webxdc,
    text: Option<&str>,
) -> anyhow::Result<MsgId> {
    let mut webxdc_msg = Message::new(Viewtype::Webxdc);
    if let Some(text) = text {
        webxdc_msg.set_text(Some(text.to_string()));
    }
    webxdc_msg.set_file(webxdc.get_str_path()?, None);
    let msg_id = chat::send_msg(context, chat_id, &mut webxdc_msg).await?;
    let conn = &mut *state.db.acquire().await?;
    db::set_webxdc_version(conn, msg_id, state.webxdc_versions.get(webxdc), webxdc).await?;
    Ok(msg_id)
}

/// Sends a [deltachat::webxdc::StatusUpdateItem] with all [AppInfo]s greater than the given serial.
/// Updating tells the frontend which apps are going to receive an updated.
pub async fn send_newest_updates(
    context: &Context,
    msg_id: MsgId,
    db: &mut SqliteConnection,
    serial: u32,
    updating: Vec<String>,
) -> anyhow::Result<()> {
    let app_infos: Vec<_> = db::get_active_app_infos_since(db, serial).await?;
    let serial = db::get_last_serial(db).await?;
    let resp = WebxdcStatusUpdatePayload::Update {
        app_infos,
        serial,
        updating,
    };
    send_update_payload_only(context, msg_id, resp).await?;
    Ok(())
}

pub async fn read_string(reader: &ZipFileReader, index: usize) -> anyhow::Result<String> {
    let mut entry = reader.reader_with_entry(index).await?;
    let mut data = String::new();
    entry.read_to_string_checked(&mut data).await?;
    Ok(data)
}

pub async fn read_vec(reader: &ZipFileReader, index: usize) -> anyhow::Result<Vec<u8>> {
    let mut entry = reader.reader_with_entry(index).await?;
    let mut data = Vec::new();
    entry.read_to_end_checked(&mut data).await?;
    Ok(data)
}

/// Sends a [deltachat::webxdc::StatusUpdateItem] with only the given payload.
pub async fn send_update_payload_only<T: Serialize>(
    context: &Context,
    msg_id: MsgId,
    payload: T,
) -> anyhow::Result<()> {
    context
        .send_webxdc_status_update_struct(
            msg_id,
            deltachat::webxdc::StatusUpdateItem {
                payload: serde_json::to_value(payload)?,
                ..Default::default()
            },
            "",
        )
        .await?;
    Ok(())
}

pub async fn get_webxdc_manifest(reader: &ZipFileReader) -> anyhow::Result<WexbdcManifest> {
    let entries = reader.file().entries();
    let manifest_index = entries
        .iter()
        .enumerate()
        .find(|(_, entry)| {
            entry
                .entry()
                .filename()
                .as_str()
                .map(|name| name == "manifest.toml")
                .unwrap_or_default()
        })
        .map(|a| a.0)
        .context("Can't find manifest.toml")?;

    Ok(toml::from_str(&read_string(reader, manifest_index).await?)?)
}

pub async fn get_webxdc_version(file: impl AsRef<Path>) -> anyhow::Result<u32> {
    let reader = ZipFileReader::new(file).await?;
    let manifest = get_webxdc_manifest(&reader).await?;
    Ok(manifest.version)
}

#[derive(Clone, Copy, Type)]
pub enum Webxdc {
    Store,
}

impl Webxdc {
    pub fn get_path(&self) -> Result<PathBuf> {
        let filename = match self {
            Webxdc::Store => STORE_XDC,
        };
        let path = project_dirs()?.config_dir().to_path_buf().join(filename);
        Ok(path)
    }

    pub fn get_str_path(&self) -> Result<String> {
        self.get_path()?
            .to_str()
            .with_context(|| format!("cannot convert path {:?} to string", self.get_path()))
            .map(|str| str.to_string())
    }

    pub fn iter() -> impl Iterator<Item = Webxdc> {
        [Webxdc::Store].iter().copied()
    }
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct WebxdcVersions {
    pub store: u32,
}

impl WebxdcVersions {
    pub fn set(&mut self, webxdc: Webxdc, version: u32) {
        match webxdc {
            Webxdc::Store => self.store = version,
        }
    }

    pub fn get(&self, webxdc: Webxdc) -> u32 {
        match webxdc {
            Webxdc::Store => self.store,
        }
    }
}

pub async fn read_webxdc_versions() -> anyhow::Result<WebxdcVersions> {
    for webxdc in Webxdc::iter() {
        let webxdc_path = webxdc.get_path().context("cannot get webxdc path")?;
        if !webxdc_path.try_exists()? {
            bail!("Required webxdc {} is not found.", webxdc_path.display());
        }
    }

    let mut futures: Vec<JoinHandle<anyhow::Result<(Webxdc, u32)>>> = vec![];
    for webxdc in Webxdc::iter() {
        futures.push(tokio::spawn(async move {
            let version = get_webxdc_version(&webxdc.get_str_path()?).await?;
            Ok((webxdc, version))
        }))
    }

    let mut versions = WebxdcVersions::default();
    for result in join_all(futures).await {
        let (webxdc, version) = result??;
        versions.set(webxdc, version);
    }
    Ok(versions)
}

#[derive(Debug, PartialEq)]
pub enum AddType {
    /// Add a new app_info
    Added,
    /// Update an existing app_info
    Updated,
    /// Ignored
    Ignored,
}

pub async fn maybe_upgrade_xdc(
    app_info: &mut AppInfo,
    conn: &mut SqliteConnection,
) -> anyhow::Result<AddType> {
    Ok(
        if db::app_version_exists(conn, &app_info.app_id, app_info.version).await? {
            AddType::Ignored
        } else if db::app_exists(conn, &app_info.app_id).await? {
            db::create_app_info(conn, app_info).await?;
            AddType::Updated
        } else {
            db::create_app_info(conn, app_info).await?;
            AddType::Added
        },
    )
}
