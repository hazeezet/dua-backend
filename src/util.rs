use std::{collections::HashMap};

use crate::{ApiResponse, ResponseError, Token};
use aws_lambda_events::{
    apigw::{ApiGatewayCustomAuthorizerPolicy, ApiGatewayCustomAuthorizerResponse, ApiGatewayProxyRequest, ApiGatewayProxyResponse},
    encodings::Body as LambdaBody,
    http::HeaderMap,
    iam::{IamPolicyEffect, IamPolicyStatement},
};
use lambda_http::{Error, LambdaEvent};
use serde::{Serialize};
use serde_json::Value;

/// Extract token and redis_id from ApiGatewayProxyRequest authorizer context
pub fn extract_token(request: &LambdaEvent<ApiGatewayProxyRequest>) -> Option<(Token, String)> {
    let context = &request.payload.request_context;
    let authorizer = &context.authorizer;

    let token_value = authorizer.fields.get("token")?;
    let redis_id_value = authorizer.fields.get("redisId")?;

    // Try to get &str from the JSON Value to avoid cloning the Value
    let token_str = token_value.as_str()?;
    let redis_id = redis_id_value.as_str()?.to_string();

    // Parse the token JSON string into the Token struct
    let token = serde_json::from_str::<Token>(token_str).ok()?;

    Some((token, redis_id))
}

/// generate lambda authorizer policy
pub fn generate_authorizer_policy(
    principal_id: &str,
    effect: &str,
    resource: &str,
    context: Option<HashMap<String, Value>>,
) -> ApiGatewayCustomAuthorizerResponse {
    let context_value = serde_json::to_value(context).unwrap_or(serde_json::json!(null));

    // Map effect string to the IamPolicyEffect enum
    let iam_effect = match effect.to_lowercase().as_str() {
        "allow" => IamPolicyEffect::Allow,
        "deny" => IamPolicyEffect::Deny,
        _ => IamPolicyEffect::Deny,
    };

    ApiGatewayCustomAuthorizerResponse {
        principal_id: Some(principal_id.to_string()),
        policy_document: ApiGatewayCustomAuthorizerPolicy {
            version: Some("2012-10-17".to_string()),
            statement: vec![IamPolicyStatement {
                action: vec!["execute-api:Invoke".to_string()],
                effect: iam_effect,
                resource: vec![resource.to_string()],
                condition: None,
            }],
        },
        context: context_value,
        usage_identifier_key: None,
    }
}

/// Build wildcard resource ARN for API Gateway
pub fn build_wildcard_arn(method_arn: &str) -> String {
    let parts: Vec<&str> = method_arn.split(':').collect();
    if parts.len() < 6 {
        return method_arn.to_string();
    }

    let api_gateway_arn = parts[5];
    let arn_parts: Vec<&str> = api_gateway_arn.split('/').collect();
    if arn_parts.len() < 3 {
        return method_arn.to_string();
    }

    let new_arn_parts = vec![arn_parts[0], arn_parts[1], "*", "*"];
    format!("{}:{}", parts[..5].join(":"), new_arn_parts.join("/"))
}

/// Create a standardized ApiGatewayProxyResponse
pub fn create_api_response<T: Serialize>(status_code: i64, response: &ApiResponse<T>) -> Result<ApiGatewayProxyResponse, Error> {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());
    
    Ok(ApiGatewayProxyResponse {
        status_code,
        headers,
        multi_value_headers: HeaderMap::new(),
        body: Some(LambdaBody::Text(serde_json::to_string(response)?)),
        is_base64_encoded: false,
    })
}

/// Create error ApiGatewayProxyResponse from ResponseError
pub fn create_api_error_response(error: ResponseError) -> Result<ApiGatewayProxyResponse, Error> {
    let response = ApiResponse::<()> {
        status_code: error.status_code,
        error: Some(error.code),
        message: error.message,
        data: None,
    };
    create_api_response(error.status_code as i64, &response)
}

/// Create success ApiGatewayProxyResponse
pub fn create_api_success_response<T: Serialize>(data: T, message: Option<&str>) -> Result<ApiGatewayProxyResponse, Error> {
    let response = ApiResponse {
        status_code: 200,
        error: None,
        message: message.unwrap_or("successful").to_string(),
        data: Some(data),
    };
    create_api_response(200, &response)
}
