//! Handlers for the different messages the bot receives
use crate::{
    db::RecordId,
    utils::{get_webxdc_manifest, read_vec},
};
use async_zip::tokio::read::fs::ZipFileReader;
use base64::encode;
use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::path::{Path, PathBuf};
use ts_rs::TS;

pub mod genisis;
pub mod shop;

#[derive(Deserialize)]
pub struct WexbdcManifest {
    /// Webxdc application identifier.
    pub app_id: String,

    /// Version of the application.
    pub version: i32,

    /// Webxdc name, used on icons or page titles.
    pub name: String,

    /// Description of the application.
    pub description: String,

    /// URL of webxdc source code.
    pub source_code_url: Option<String>,

    /// Uri of the submitter.
    pub submitter_uri: Option<String>,
}

#[derive(TS, Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
#[ts(export)]
#[ts(export_to = "frontend/src/bindings/")]
pub struct AppInfo {
    #[serde(skip)]
    pub id: RecordId,
    pub app_id: String,                  // manifest
    pub version: i32,                    // manifest
    pub name: String,                    // manifest
    pub submitter_uri: Option<String>,   // bot
    pub source_code_url: Option<String>, // manifest
    pub image: String,                   // webxdc
    pub description: String,             // submit
    #[serde(skip)]
    pub xdc_blob_path: PathBuf, // bot
    #[serde(skip)]
    pub originator: RecordId, // bot
}

impl AppInfo {
    /// Create appinfo from webxdc file.
    pub async fn from_xdc(file: &Path) -> anyhow::Result<Self> {
        let mut app = AppInfo {
            xdc_blob_path: file.to_path_buf(),
            ..Default::default()
        };
        app.update_from_xdc(file.to_path_buf()).await?;
        Ok(app)
    }

    /// Reads a webxdc file and overwrites current fields.
    /// Returns true if the version has changed.
    pub async fn update_from_xdc(&mut self, file: PathBuf) -> anyhow::Result<bool> {
        let mut upgraded = false;
        let reader = ZipFileReader::new(&file).await?;
        let entries = reader.file().entries();
        let manifest = get_webxdc_manifest(&reader).await?;

        self.app_id = manifest.app_id;
        if self.version != manifest.version {
            upgraded = true
        }
        self.version = manifest.version;
        self.name = manifest.name;
        self.description = manifest.description;
        self.source_code_url = manifest.source_code_url;
        self.submitter_uri = manifest.submitter_uri;
        // self.submission_date = manifest.submission_date;

        self.xdc_blob_path = file;

        let icon = entries
            .iter()
            .enumerate()
            .find(|(_, entry)| {
                entry
                    .entry()
                    .filename()
                    .as_str()
                    .map(|name| name == "icon.png")
                    .unwrap_or_default()
            })
            .map(|a| a.0);

        if let Some(index) = icon {
            let res = read_vec(&reader, index).await?;
            self.image = encode(&res)
        }
        Ok(upgraded)
    }
}

#[derive(Serialize, Deserialize, Type, Clone, Copy, Debug, PartialEq)]

pub enum ChatType {
    Shop,
    Genesis,
}

/// A generic webxdc update
#[derive(Deserialize)]
pub struct WebxdcStatusUpdate<T> {
    pub payload: T,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
#[ts(export_to = "frontend/src/bindings/")]
#[serde(tag = "type")]

pub enum GeneralFrontendResponse {
    Outdated { critical: bool, version: i32 },
    UpdateSent,
}

#[derive(Deserialize, TS)]
#[ts(export)]
#[ts(export_to = "frontend/src/bindings/")]
#[serde(tag = "type")]
pub enum GeneralFrontendRequest {
    UpdateWebxdc,
}
