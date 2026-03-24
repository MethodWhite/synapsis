//! JSON-RPC 2.0 utilities for Synapsis
//!
//! Common request/response handling for both TCP and stdio transports.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// JSON-RPC 2.0 Request
#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version (should be "2.0")
    pub jsonrpc: Option<String>,
    /// Method name
    pub method: String,
    /// Parameters (object or array)
    pub params: Option<Value>,
    /// Request ID (string, number, or null)
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version (should be "2.0")
    pub jsonrpc: Option<String>,
    /// Result on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// Request ID (same as request)
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 Error object
#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcRequest {
    /// Parse a JSON-RPC request from a string
    pub fn parse(input: &str) -> Result<Self, String> {
        serde_json::from_str(input).map_err(|e| format!("Invalid JSON: {}", e))
    }

    /// Get the method name
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Get parameters as Value
    pub fn params(&self) -> Option<&Value> {
        self.params.as_ref()
    }

    /// Get request ID
    pub fn id(&self) -> Option<&Value> {
        self.id.as_ref()
    }
}

impl JsonRpcResponse {
    /// Create a successful response
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: Some("2.0".to_string()),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(id: Option<Value>, code: i32, message: &str, data: Option<Value>) -> Self {
        Self {
            jsonrpc: Some("2.0".to_string()),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data,
            }),
            id,
        }
    }

    /// Convert to JSON string
    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error"},"id":null}"#.to_string())
    }

    /// Create a parse error response (invalid JSON)
    pub fn parse_error() -> Self {
        Self::error(
            None,
            -32700,
            "Parse error",
            None,
        )
    }

    /// Create a method not found error
    pub fn method_not_found(id: Option<Value>) -> Self {
        Self::error(
            id,
            -32601,
            "Method not found",
            None,
        )
    }

    /// Create an invalid params error
    pub fn invalid_params(id: Option<Value>, message: Option<&str>) -> Self {
        Self::error(
            id,
            -32602,
            message.unwrap_or("Invalid params"),
            None,
        )
    }

    /// Create an internal error response
    pub fn internal_error(id: Option<Value>, message: &str) -> Self {
        Self::error(
            id,
            -32603,
            message,
            None,
        )
    }
}

/// Parse a line into a JSON-RPC request and produce a response using a handler function
pub fn handle_request<F>(line: &str, handler: F) -> String
where
    F: FnOnce(JsonRpcRequest) -> JsonRpcResponse,
{
    match JsonRpcRequest::parse(line) {
        Ok(req) => handler(req).to_string(),
        Err(_) => JsonRpcResponse::parse_error().to_string(),
    }
}

/// Common error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const SERVER_ERROR: i32 = -32000;
}

/// Helper to extract a string parameter from params
pub fn get_string_param(params: Option<&Value>, key: &str) -> Option<String> {
    params?
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Helper to extract an optional string parameter
pub fn get_optional_string_param(params: Option<&Value>, key: &str) -> Option<String> {
    params?
        .get(key)
        .and_then(|v| {
            if v.is_null() {
                None
            } else {
                v.as_str().map(|s| s.to_string())
            }
        })
}

/// Helper to extract an integer parameter
pub fn get_i64_param(params: Option<&Value>, key: &str) -> Option<i64> {
    params?
        .get(key)
        .and_then(|v| v.as_i64())
}

/// Helper to extract a boolean parameter
pub fn get_bool_param(params: Option<&Value>, key: &str) -> Option<bool> {
    params?
        .get(key)
        .and_then(|v| v.as_bool())
}

/// Trait for handling JSON-RPC requests
pub trait RpcHandler: Send + Sync {
    /// Handle a JSON-RPC request and return a response
    fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse;
    
    /// Helper method to parse string input and handle request
    fn handle_input(&self, input: &str) -> String {
        match JsonRpcRequest::parse(input) {
            Ok(req) => self.handle_request(req).to_string(),
            Err(_) => JsonRpcResponse::parse_error().to_string(),
        }
    }
}

/// Basic implementation for closures
impl<F> RpcHandler for F
where
    F: Fn(JsonRpcRequest) -> JsonRpcResponse + Send + Sync,
{
    fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        self(req)
    }
}