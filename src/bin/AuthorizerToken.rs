use aws_lambda_events::apigw::{
    ApiGatewayCustomAuthorizerRequest, ApiGatewayCustomAuthorizerResponse,
};
use aws_sdk_dynamodb::Client;
use ephelion_example::database::redis::{RedisDB};
use ephelion_example::encryption::Encryption;
use ephelion_example::ssm::SsmParameter;
use ephelion_example::util::{build_wildcard_arn, generate_authorizer_policy};
use ephelion_example::Token;
use lambda_http::tracing;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde_json::Value;
use std::collections::HashMap;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    // Initialize the AWS SDK for Rust
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;

    let redis_table_name = env::var("REDIS_TABLE").expect("REDIS_TABLE must be set");

    let dynamodb_client = Client::new(&config);

    let stage = env::var("STAGE").unwrap_or_else(|_| "dev".to_string());

    let cookie_secret = format!("/ephelion/{}/cookie_secret", stage);

    let ssm = SsmParameter::new().await?;

    let secret = ssm.get_parameter_value(&cookie_secret, Some(true)).await?;

    let encryption = Encryption::new(secret).expect("Failed to create encryption service");

    run(service_fn(
        |event: LambdaEvent<ApiGatewayCustomAuthorizerRequest>| {
            authorize(event, &dynamodb_client, &encryption, &redis_table_name)
        },
    ))
    .await?;

    Ok(())
}

async fn authorize(
    event: LambdaEvent<ApiGatewayCustomAuthorizerRequest>,
    client: &Client,
    encryption: &Encryption,
    redis_table_name: &str,
) -> Result<ApiGatewayCustomAuthorizerResponse, Error> {
    // Extract session from cookies or authorization header
    let redis_id = event.payload.authorization_token.clone();

    let method_arn = event.payload.method_arn.clone();

    let method_arn = match method_arn {
        Some(m) => m,
        None => {
            tracing::error!(event = ?event.payload, "No method ARN found in the request");
            return Ok(generate_authorizer_policy(
                "unauthorized",
                "Deny",
                &build_wildcard_arn("*"),
                None::<HashMap<String, serde_json::Value>>,
            ));
        }
    };

    match redis_id {
        Some(redis_id_str) => {
            let redis_id_str = encryption.decrypt(&redis_id_str)?;

            match authenticate(&redis_id_str, client, redis_table_name).await {
                Ok(token) => {
                    let token_string = serde_json::to_string(&token).unwrap_or_default();

                    // Create context to pass to the lambda function
                    let mut context = HashMap::new();

                    context.insert("token".to_string(), Value::String(token_string));
                    context.insert("redisId".to_string(), Value::String(redis_id_str));

                    // Determine principal id from token username or use a default
                    let principal_id = token.username;

                    let policy = generate_authorizer_policy(
                        &principal_id,
                        "Allow",
                        &build_wildcard_arn(&method_arn),
                        Some(context),
                    );

                    tracing::info!(principal_id = ?principal_id, policy = ?policy, "Authorization successful");

                    Ok(policy)
                }
                Err(_auth_error) => Ok(generate_authorizer_policy(
                    "unauthorized",
                    "Deny",
                    &build_wildcard_arn("*"),
                    None::<HashMap<String, serde_json::Value>>,
                )),
            }
        }
        None => Ok({
            tracing::error!(
                authorization_token = ?event,
                "No authorization token found in the request"
            );
            generate_authorizer_policy(
                "unauthorized",
                "Deny",
                &build_wildcard_arn("*"),
                None::<HashMap<String, serde_json::Value>>,
            )
        }),
    }
}

async fn authenticate(
    redis_id: &str,
    client: &Client,
    redis_table_name: &str,
) -> Result<Token, Error> {
    // Initialize Redis DB connection
    let redis_db = RedisDB::new(client.clone(), redis_table_name.to_string());

    // Get user token from Redis
    let user_redis_token = match redis_db.get_token(redis_id).await {
        Ok(Some(token)) => token,

        Ok(None) => {
            return Err(Error::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Your session has expired. Please log in again",
            )));
        }
        Err(error) => {
            tracing::error!(error = %error, "Authorization error");

            return Err(Error::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unable to validate your session",
            )));
        }
    };

    // Return redis_id and user result
    Ok(user_redis_token)
}
