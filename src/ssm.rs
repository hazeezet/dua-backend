use aws_sdk_ssm::error::SdkError;
use aws_sdk_ssm::operation::{
    get_parameter::GetParameterError,
    get_parameters::GetParametersError,
    get_parameters_by_path::GetParametersByPathError,
};
use aws_sdk_ssm::{Client};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum SsmError {
    #[error("SSM GetParameter error: {0}")]
    GetParameter(#[from] SdkError<GetParameterError>),
    #[error("SSM GetParameters error: {0}")]
    GetParameters(#[from] SdkError<GetParametersError>),
    #[error("SSM GetParametersByPath error: {0}")]
    GetParametersByPath(#[from] SdkError<GetParametersByPathError>),
    #[error("Parameter not found: {0}")]
    ParameterNotFound(String),
    #[error("Invalid parameter value: {0}")]
    InvalidParameterValue(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("AWS configuration error: {0}")]
    AwsConfig(String),
}

/// Configuration for SSM Parameter Store
#[derive(Debug, Clone)]
pub struct SsmConfig {
    /// AWS region for SSM
    pub region: String,
    /// Optional prefix for parameter names
    pub parameter_prefix: Option<String>,
}

impl Default for SsmConfig {
    fn default() -> Self {
        Self {
            region: "eu-west-1".to_string(),
            parameter_prefix: None,
        }
    }
}

/// SSM Parameter Store service for retrieving parameters
pub struct SsmParameter {
    client: Client,
    config: SsmConfig,
}

/// Represents a parameter retrieved from SSM
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    /// The name of the parameter
    pub name: String,
    /// The value of the parameter
    pub value: String,
    /// The parameter type (String, StringList, or SecureString)
    pub parameter_type: String,
    /// The version of the parameter
    pub version: Option<i64>,
    /// The last modified date
    pub last_modified_date: Option<String>,
}

/// Request structure for getting a parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetParameterRequest {
    /// The name of the parameter to retrieve
    pub name: String,
    /// Whether to decrypt the parameter value (for SecureString parameters)
    pub with_decryption: Option<bool>,
}

/// Request structure for getting multiple parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetParametersRequest {
    /// The names of the parameters to retrieve
    pub names: Vec<String>,
    /// Whether to decrypt the parameter values (for SecureString parameters)
    pub with_decryption: Option<bool>,
}

/// Request structure for getting parameters by path
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetParametersByPathRequest {
    /// The path of the parameters to retrieve
    pub path: String,
    /// Whether to decrypt the parameter values (for SecureString parameters)
    pub with_decryption: Option<bool>,
    /// Whether to retrieve parameters recursively
    pub recursive: Option<bool>,
    /// Maximum number of parameters to retrieve
    pub max_results: Option<i32>,
    /// Token for pagination
    pub next_token: Option<String>,
}

/// Response structure for getting parameters by path
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetParametersByPathResponse {
    /// The retrieved parameters
    pub parameters: Vec<Parameter>,
    /// Token for pagination
    pub next_token: Option<String>,
}

impl SsmParameter {
    /// Create a new SSM parameter service with default configuration
    pub async fn new() -> Result<Self, SsmError> {
        let config = SsmConfig::default();
        Self::with_config(config).await
    }

    /// Create a new SSM parameter service with custom configuration
    pub async fn with_config(config: SsmConfig) -> Result<Self, SsmError> {
        let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;

        let client = Client::new(&aws_config);

        Ok(Self { client, config })
    }

    /// Add prefix to parameter name if configured
    fn get_parameter_name(&self, name: &str) -> String {
        match &self.config.parameter_prefix {
            Some(prefix) => {
                if name.starts_with('/') {
                    format!("{}{}", prefix, name)
                } else {
                    format!("{}/{}", prefix, name)
                }
            }
            None => name.to_string(),
        }
    }

    /// Get a single parameter from SSM Parameter Store
    pub async fn get_parameter(
        &self,
        request: GetParameterRequest,
    ) -> Result<Parameter, SsmError> {
        let parameter_name = self.get_parameter_name(&request.name);
        
        let mut get_param_builder = self.client.get_parameter().name(&parameter_name);

        if let Some(with_decryption) = request.with_decryption {
            get_param_builder = get_param_builder.with_decryption(with_decryption);
        }

        let response = get_param_builder
            .send()
            .await?;

        let parameter = response
            .parameter()
            .ok_or_else(|| SsmError::ParameterNotFound(parameter_name.clone()))?;

        Ok(Parameter {
            name: parameter.name().unwrap_or("").to_string(),
            value: parameter.value().unwrap_or("").to_string(),
            parameter_type: parameter.r#type().map(|t| format!("{:?}", t)).unwrap_or_default(),
            version: Some(parameter.version()),
            last_modified_date: parameter
                .last_modified_date()
                .map(|date| date.fmt(aws_smithy_types::date_time::Format::DateTime).unwrap_or_default()),
        })
    }

    /// Get a single parameter value as string (convenience method)
    pub async fn get_parameter_value(
        &self,
        name: &str,
        with_decryption: Option<bool>,
    ) -> Result<String, SsmError> {
        let request = GetParameterRequest {
            name: name.to_string(),
            with_decryption,
        };

        let parameter = self.get_parameter(request).await?;
        Ok(parameter.value)
    }

    /// Get multiple parameters from SSM Parameter Store
    pub async fn get_parameters(
        &self,
        request: GetParametersRequest,
    ) -> Result<Vec<Parameter>, SsmError> {
        let parameter_names: Vec<String> = request
            .names
            .iter()
            .map(|name| self.get_parameter_name(name))
            .collect();

        let mut get_params_builder = self.client.get_parameters().set_names(Some(parameter_names.clone()));

        if let Some(with_decryption) = request.with_decryption {
            get_params_builder = get_params_builder.with_decryption(with_decryption);
        }

        let response = get_params_builder
            .send()
            .await?;

        let mut parameters = Vec::new();

        for parameter in response.parameters() {
            parameters.push(Parameter {
                name: parameter.name().unwrap_or("").to_string(),
                value: parameter.value().unwrap_or("").to_string(),
                parameter_type: parameter.r#type().map(|t| format!("{:?}", t)).unwrap_or_default(),
                version: Some(parameter.version()),
                last_modified_date: parameter
                    .last_modified_date()
                    .map(|date| date.fmt(aws_smithy_types::date_time::Format::DateTime).unwrap_or_default()),
            });
        }

        // Check for invalid parameters and return error if any
        if !response.invalid_parameters().is_empty() {
            return Err(SsmError::ParameterNotFound(
                format!("Invalid parameters: {:?}", response.invalid_parameters())
            ));
        }

        Ok(parameters)
    }

    /// Get parameters by path from SSM Parameter Store
    pub async fn get_parameters_by_path(
        &self,
        request: GetParametersByPathRequest,
    ) -> Result<GetParametersByPathResponse, SsmError> {
        let path = self.get_parameter_name(&request.path);

        let mut get_params_builder = self.client.get_parameters_by_path().path(&path);

        if let Some(with_decryption) = request.with_decryption {
            get_params_builder = get_params_builder.with_decryption(with_decryption);
        }

        if let Some(recursive) = request.recursive {
            get_params_builder = get_params_builder.recursive(recursive);
        }

        if let Some(max_results) = request.max_results {
            get_params_builder = get_params_builder.max_results(max_results);
        }

        if let Some(next_token) = &request.next_token {
            get_params_builder = get_params_builder.next_token(next_token);
        }

        let response = get_params_builder
            .send()
            .await?;

        let mut parameters = Vec::new();

        for parameter in response.parameters() {
            parameters.push(Parameter {
                name: parameter.name().unwrap_or("").to_string(),
                value: parameter.value().unwrap_or("").to_string(),
                parameter_type: parameter.r#type().map(|t| format!("{:?}", t)).unwrap_or_default(),
                version: Some(parameter.version()),
                last_modified_date: parameter
                    .last_modified_date()
                    .map(|date| date.fmt(aws_smithy_types::date_time::Format::DateTime).unwrap_or_default()),
            });
        }

        Ok(GetParametersByPathResponse {
            parameters,
            next_token: response.next_token().map(|s| s.to_string()),
        })
    }

    /// Get all parameters by path (handles pagination automatically)
    pub async fn get_all_parameters_by_path(
        &self,
        path: &str,
        with_decryption: Option<bool>,
        recursive: Option<bool>,
    ) -> Result<Vec<Parameter>, SsmError> {
        let mut all_parameters = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let request = GetParametersByPathRequest {
                path: path.to_string(),
                with_decryption,
                recursive,
                max_results: Some(10), // AWS default max per request
                next_token: next_token.clone(),
            };

            let response = self.get_parameters_by_path(request).await?;
            all_parameters.extend(response.parameters);

            if response.next_token.is_none() {
                break;
            }
            next_token = response.next_token;
        }

        Ok(all_parameters)
    }

    /// Get parameters as a HashMap for easy access
    pub async fn get_parameters_as_map(
        &self,
        names: &[&str],
        with_decryption: Option<bool>,
    ) -> Result<HashMap<String, String>, SsmError> {
        let request = GetParametersRequest {
            names: names.iter().map(|s| s.to_string()).collect(),
            with_decryption,
        };

        let parameters = self.get_parameters(request).await?;
        let mut map = HashMap::new();

        for parameter in parameters {
            // Remove prefix from parameter name for the map key
            let key = if let Some(prefix) = &self.config.parameter_prefix {
                parameter.name.strip_prefix(prefix).unwrap_or(&parameter.name).to_string()
            } else {
                parameter.name
            };
            map.insert(key, parameter.value);
        }

        Ok(map)
    }
}

/// Example usage
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ssm_parameter_operations() {
        // This is an example of how to use the SSM service
        // Note: These tests require AWS credentials and actual parameters in SSM
        
        // Initialize SSM service
        let ssm = SsmParameter::new().await.unwrap();

        // Get a single parameter
        let request = GetParameterRequest {
            name: "/myapp/database/password".to_string(),
            with_decryption: Some(true),
        };
        
        match ssm.get_parameter(request).await {
            Ok(parameter) => {
                println!("Parameter: {} = {}", parameter.name, parameter.value);
            }
            Err(e) => {
                println!("Error getting parameter: {}", e);
            }
        }

        // Get multiple parameters
        let request = GetParametersRequest {
            names: vec![
                "/myapp/database/host".to_string(),
                "/myapp/database/port".to_string(),
            ],
            with_decryption: Some(false),
        };

        match ssm.get_parameters(request).await {
            Ok(parameters) => {
                for param in parameters {
                    println!("Parameter: {} = {}", param.name, param.value);
                }
            }
            Err(e) => {
                println!("Error getting parameters: {}", e);
            }
        }

        // Get parameters by path
        let request = GetParametersByPathRequest {
            path: "/myapp/database/".to_string(),
            with_decryption: Some(true),
            recursive: Some(true),
            max_results: None,
            next_token: None,
        };

        match ssm.get_parameters_by_path(request).await {
            Ok(response) => {
                for param in response.parameters {
                    println!("Parameter: {} = {}", param.name, param.value);
                }
            }
            Err(e) => {
                println!("Error getting parameters by path: {}", e);
            }
        }
    }
}
