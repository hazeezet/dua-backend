use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::{
    delete_item::DeleteItemError, get_item::GetItemError, put_item::PutItemError,
    query::QueryError, update_item::UpdateItemError,
};
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use serde_dynamo;

use crate::{Token, User};

pub struct RedisDB {
    client: Client,
    table_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("DynamoDB Query error: {0}")]
    Query(#[from] SdkError<QueryError>),
    #[error("DynamoDB GetItem error: {0}")]
    GetItem(#[from] SdkError<GetItemError>),
    #[error("DynamoDB PutItem error: {0}")]
    PutItem(#[from] SdkError<PutItemError>),
    #[error("DynamoDB UpdateItem error: {0}")]
    UpdateItem(#[from] SdkError<UpdateItemError>),
    #[error("DynamoDB DeleteItem error: {0}")]
    DeleteItem(#[from] SdkError<DeleteItemError>),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_dynamo::Error),
}

impl RedisDB {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    pub async fn get_token(&self, redis_id: &str) -> Result<Option<Token>, DatabaseError> {
        let result = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("mainId", AttributeValue::S(redis_id.to_string()))
            .key("sortId", AttributeValue::S("TOKEN".to_string()))
            .send()
            .await?;

        if let Some(item) = result.item {
            let token: Token = serde_dynamo::from_item(item)?;

            Ok(Some(token))
        } else {
            Ok(None)
        }
    }
}

pub struct UserDB {
    client: Client,
    table_name: String,
}

impl UserDB {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    /// Get user by ID
    pub async fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>, DatabaseError> {
        let result = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("mainId", AttributeValue::S(user_id.to_string()))
            .key("sortId", AttributeValue::S("USER".to_string()))
            .send()
            .await?;

        match result.item {
            Some(item) => {
                let user: User = serde_dynamo::from_item(item)?;

                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    /// Get user by email
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, DatabaseError> {
        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("EmailIndex")
            .key_condition_expression("email = :email")
            .expression_attribute_values(":email", AttributeValue::S(email.to_string()))
            .send()
            .await?;

        match result.items {
            Some(items) if !items.is_empty() => {
                let user: User = serde_dynamo::from_item(items[0].clone())?;
                Ok(Some(user))
            }
            _ => Ok(None),
        }
    }

    /// Get user by username
    pub async fn get_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<User>, DatabaseError> {
        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("UsernameIndex")
            .key_condition_expression("username = :username")
            .expression_attribute_values(":username", AttributeValue::S(username.to_string()))
            .send()
            .await?;

        match result.items {
            Some(items) if !items.is_empty() => {
                let user: User = serde_dynamo::from_item(items[0].clone())?;
                Ok(Some(user))
            }
            _ => Ok(None),
        }
    }
}
