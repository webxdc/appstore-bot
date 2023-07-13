//! Handlers for the different messages the bot receives
use crate::{
    db::RecordId,
    utils::{get_webxdc_manifest, read_vec},
};
use async_zip::tokio::read::fs::ZipFileReader;
use base64::encode;
use serde::{Deserialize, Serialize};
use sqlx::{Decode, FromRow, Type};
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::fs::File;
use ts_rs::TS;

pub mod genesis;
pub mod store;

#[derive(Deserialize)]
pub struct WexbdcManifest {
    /// Webxdc application identifier.
    pub app_id: String,

    /// Version of the application.
    pub version: u32,

    /// Webxdc name, used on icons or page titles.
    pub name: String,

    /// Description of the application.
    pub description: String,

    /// URL of webxdc source code.
    pub source_code_url: Option<String>,

    /// Uri of the submitter.
    pub submitter_uri: Option<String>,

    /// Date displayed in the store.
    pub date: String,
}

#[derive(TS, Deserialize, Serialize, Clone, Debug, Default, PartialEq, FromRow, Decode)]
#[ts(export)]
#[ts(export_to = "frontend/src/bindings/")]
pub struct AppInfo {
    #[serde(skip)]
    pub id: RecordId,
    pub app_id: String, // manifest
    pub version: u32,   // manifest
    pub date: i64,      // manifest
    pub name: String,   // manifest
    #[serde(skip)]
    pub submitter_uri: Option<String>, // bot
    pub source_code_url: Option<String>, // manifest
    pub image: String,  // webxdc
    pub description: String, // submit
    #[serde(skip)]
    pub xdc_blob_path: PathBuf, // bot
    pub size: i64,      //bot
}

impl AppInfo {
    /// Create appinfo from webxdc file.
    pub async fn from_xdc(file: &Path) -> anyhow::Result<Self> {
        let size = i64::try_from(File::open(&file).await?.metadata().await?.len())?;
        let reader = ZipFileReader::new(&file).await?;
        let entries = reader.file().entries();
        let manifest = get_webxdc_manifest(&reader).await?;

        let image = entries
            .iter()
            .enumerate()
            .find(|(_, entry)| {
                entry
                    .entry()
                    .filename()
                    .as_str()
                    .map(|name| name == "icon.png" || name == "icon.jpg")
                    .unwrap_or_default()
            })
            .map(|a| a.0);
        let image = if let Some(index) = image {
            let res = read_vec(&reader, index).await?;
            Ok(encode(&res))
        } else {
            Err(anyhow::anyhow!("Could not find image"))
        };

        Ok(Self {
            size,
            date: OffsetDateTime::parse(&manifest.date, &Rfc3339)?.unix_timestamp(),
            app_id: manifest.app_id,
            version: manifest.version,
            name: manifest.name,
            submitter_uri: manifest.submitter_uri,
            source_code_url: manifest.source_code_url,
            image: image?,
            description: manifest.description,
            xdc_blob_path: file.to_path_buf(),
            id: 0, // This will be updated by the db on insert
        })
    }
}

#[derive(Serialize, Deserialize, Type, Clone, Copy, Debug, PartialEq)]

pub enum ChatType {
    Store,
    Genesis,
}

/// WebXDC status update.
#[derive(Serialize, Deserialize)]
pub struct WebxdcStatusUpdate {
    pub payload: WebxdcStatusUpdatePayload,
}

/// WebXDC status update payload.
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
#[ts(export_to = "frontend/src/bindings/")]
#[serde(tag = "type")]
pub enum WebxdcStatusUpdatePayload {
    // General update request.
    UpdateWebxdc,

    // General update response.
    Outdated {
        critical: bool,
        version: u32,
    },
    UpdateSent,

    // Store WebXDC requests.
    UpdateRequest {
        /// Requested update sequence number.
        serial: u32,
        /// List of apps selected for caching.
        #[serde(default)]
        apps: Vec<(String, u32)>,
    },
    Download {
        /// ID of the requested application.
        app_id: String,
    },

    // Store bot responses.
    DownloadOkay {
        /// app_id of the downloaded app.
        app_id: String,
        /// Name to be used as filename in `sendToChat`.
        name: String,
        /// Base64 encoded webxdc.
        data: String,
    },
    DownloadError {
        app_id: String,
        error: String,
    },
    Update {
        /// List of new / updated app infos.
        app_infos: Vec<AppInfo>,
        serial: i32,
        /// `app_id`s of apps that will receive an update.
        /// The frontend can use these to set the state to updating.
        updating: Vec<String>,
    },
}
