use crate::{ApiResponse, ResponseError};
use aws_lambda_events::{
    apigw::ApiGatewayProxyResponse, encodings::Body as LambdaBody, http::HeaderMap,
};
use lambda_http::Error;
use serde::Serialize;

/// Create a standardized API Gateway response with CORS headers
pub fn create_api_response<T: Serialize>(
    status_code: i64,
    response: &ApiResponse<T>,
) -> Result<ApiGatewayProxyResponse, Error> {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers.insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    headers.insert(
        "Access-Control-Allow-Headers",
        "Content-Type".parse().unwrap(),
    );
    headers.insert(
        "Access-Control-Allow-Methods",
        "POST, OPTIONS".parse().unwrap(),
    );

    Ok(ApiGatewayProxyResponse {
        status_code,
        headers,
        multi_value_headers: HeaderMap::new(),
        body: Some(LambdaBody::Text(serde_json::to_string(response)?)),
        is_base64_encoded: false,
    })
}

/// Create error response from ResponseError
pub fn create_api_error_response(error: ResponseError) -> Result<ApiGatewayProxyResponse, Error> {
    let response = ApiResponse::<()> {
        status_code: error.status_code,
        error: Some(error.code),
        message: error.message,
        data: None,
    };
    create_api_response(error.status_code as i64, &response)
}

/// Create success response with data
pub fn create_api_success_response<T: Serialize>(
    data: T,
    message: Option<&str>,
) -> Result<ApiGatewayProxyResponse, Error> {
    let response = ApiResponse {
        status_code: 200,
        error: None,
        message: message.unwrap_or("successful").to_string(),
        data: Some(data),
    };
    create_api_response(200, &response)
}
