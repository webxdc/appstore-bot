//! Integration fo SurrealDBpub struct DB
use deltachat::{chat::ChatId, contact::ContactId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb::{
    engine::local::{Db, File},
    sql::Thing,
    Surreal,
};

use crate::{
    bot::BotConfig,
    request_handlers::{review::ReviewChat, submit::SubmitChat, AppInfo, AppInfoId, ChatType},
};

#[derive(Serialize, Deserialize)]
struct DBChatType {
    chat_type: ChatType,
}

#[derive(Serialize, Deserialize)]
struct DBContactId {
    contact_id: ContactId,
}

pub struct DB {
    db: Surreal<Db>,
}

#[allow(unused)]
impl DB {
    pub async fn new(store: &str) -> Self {
        let db = Surreal::new::<File>(store).await.unwrap();
        db.use_ns("bot").use_db("bot").await.unwrap();
        Self { db }
    }

    pub async fn get_review_chat(&self, chat_id: ChatId) -> surrealdb::Result<Option<ReviewChat>> {
        self.db.select(("chat", chat_id.to_u32().to_string())).await
    }


    pub async fn get_submit_chat(&self, chat_id: ChatId) -> surrealdb::Result<Option<SubmitChat>> {
        self.db.select(("chat", chat_id.to_u32().to_string())).await
    }

    pub async fn create_submit(&self, chat: &SubmitChat) -> surrealdb::Result<SubmitChat> {
        let res = self
            .db
            .create(("chat", chat.creator_chat.to_u32().to_string()))
            .content(chat)
            .await?;
        Ok(res.unwrap())
    }

    pub async fn upgrade_to_review_chat(&self, chat: &ReviewChat) -> surrealdb::Result<()> {
        let res: Option<ReviewChat> = self
            .db
            .update(("chat", chat.creator_chat.to_u32().to_string()))
            .content(chat)
            .await?;
        Ok(())
    }

    pub async fn set_chat_type(
        &self,
        chat_id: ChatId,
        chat_type: ChatType,
    ) -> surrealdb::Result<()> {
        let _t: DBChatType = self
            .db
            .create(("chattype", chat_id.to_u32().to_string()))
            .content(DBChatType { chat_type })
            .await?
            .unwrap();
        Ok(())
    }

    pub async fn get_chat_type(&self, chat_id: ChatId) -> surrealdb::Result<Option<ChatType>> {
        let c: Result<Option<DBChatType>, _> = self
            .db
            .select(("chattype", chat_id.to_u32().to_string()))
            .await;
        c.map(|a| a.map(|a| a.chat_type))
    }

    pub async fn add_contact_to_genesis(&self, contact_id: ContactId) -> surrealdb::Result<()> {
        let _t: DBContactId = self
            .db
            .create(("genesis", contact_id.to_u32().to_string()))
            .content(DBContactId { contact_id })
            .await?
            .unwrap();
        Ok(())
    }

    pub async fn set_genesis_contacts(&self, contacts: &[ContactId]) -> surrealdb::Result<()> {
        let _t: Vec<DBContactId> = self.db.delete("genesis").await?;
        for contact_id in contacts {
            self.add_contact_to_genesis(*contact_id).await?;
        }
        Ok(())
    }

    pub async fn create_publisher(&self, contact_id: ContactId) -> surrealdb::Result<()> {
        let _t: DBContactId = self
            .db
            .create(("publisher", contact_id.to_u32().to_string()))
            .content(DBContactId { contact_id })
            .await?
            .unwrap();
        Ok(())
    }

    pub async fn set_publisher_contacts(&self, contacts: &[ContactId]) -> surrealdb::Result<()> {
        let _t: Vec<DBContactId> = self.db.delete("publisher").await?;
        for contact_id in contacts {
            self.create_publisher(*contact_id).await?;
        }
        Ok(())
    }

    pub async fn get_publisher(&self) -> surrealdb::Result<Option<ContactId>> {
        let mut result = self
            .db
            .query("SELECT contact_id FROM publisher LIMIT 1")
            .await?;
        let contact_id: Vec<ContactId> = result.take((0, "contact_id")).unwrap();
        Ok(contact_id.get(0).copied())
    }

    pub async fn create_tester(&self, contact_id: ContactId) -> surrealdb::Result<()> {
        let _t: DBContactId = self
            .db
            .create(("testers", contact_id.to_u32().to_string()))
            .content(DBContactId { contact_id })
            .await?
            .unwrap();
        Ok(())
    }

    pub async fn set_tester_contacts(&self, contacts: &[ContactId]) -> surrealdb::Result<()> {
        let _t: Vec<DBContactId> = self.db.delete("testers").await?;
        for contact_id in contacts {
            self.create_tester(*contact_id).await?;
        }
        Ok(())
    }

    pub async fn get_testers(&self) -> surrealdb::Result<Vec<ContactId>> {
        let mut result = self
            .db
            .query("SELECT contact_id FROM testers LIMIT 3")
            .await?;

        let testers = result.take::<Vec<ContactId>>((0, "contact_id")).unwrap();
        Ok(testers)
    }

    pub async fn set_config(&self, config: &BotConfig) -> surrealdb::Result<BotConfig> {
        let _t: Option<BotConfig> = self.db.delete(("config", "config")).await.ok().flatten();
        let res = self.db.create(("config", "config")).content(config).await?;
        Ok(res.unwrap())
    }

    pub async fn get_config(&self) -> surrealdb::Result<Option<BotConfig>> {
        let res = self.db.select(("config", "config")).await?;
        Ok(res)
    }

    pub async fn create_app_info(
        &self,
        app_info: &AppInfo,
        resource_id: Thing,
    ) -> surrealdb::Result<AppInfo> {
        let res = self.db.create(resource_id).content(app_info).await?;
        Ok(res.unwrap())
    }

    pub async fn update_app_info(
        &self,
        app_info: &AppInfo,
        id: &Thing,
    ) -> surrealdb::Result<AppInfo> {
        let res = self.db.update(id.clone()).content(app_info).await?;
        Ok(res.unwrap())
    }

    pub async fn publish_app(&self, id: &Thing) -> surrealdb::Result<AppInfo> {
        let res = self
            .db
            .update(id.clone())
            .merge(json!({"active": true}))
            .await?;
        Ok(res.unwrap())
    }

    pub async fn get_app_info(&self, resource_id: &Thing) -> surrealdb::Result<AppInfo> {
        let res = self.db.select(resource_id.clone()).await?;
        Ok(res.unwrap())
    }

    pub async fn get_active_app_infos(&self) -> surrealdb::Result<Vec<AppInfoId>> {
        let mut result = self.db.query("select * from app_info").await?;
        let testers = result.take::<Vec<AppInfoId>>(0)?;
        Ok(testers)
    }
}
