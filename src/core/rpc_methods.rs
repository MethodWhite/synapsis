//! RPC Method Handlers for Synapsis
//!
//! Unified handling of JSON-RPC methods across different server implementations.

use crate::core::json_rpc::{
    get_bool_param, get_i64_param, get_optional_string_param, get_string_param, JsonRpcRequest,
    JsonRpcResponse,
};
use serde_json::Value;

/// Trait for RPC method handlers
pub trait RpcMethodHandler {
    /// Handle a ping request
    fn handle_ping(&self, req: &JsonRpcRequest) -> JsonRpcResponse {
        JsonRpcResponse::success(req.id().cloned(), serde_json::json!({"status": "ok"}))
    }

    /// Handle session registration
    fn handle_session_register(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle session reconnect
    fn handle_session_reconnect(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle agent heartbeat
    fn handle_agent_heartbeat(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle send message
    fn handle_send_message(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle get active agents
    fn handle_agents_active(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle skill list
    fn handle_skill_list(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle skill activation
    fn handle_skill_activate(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle skill registration
    fn handle_skill_register(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task creation
    fn handle_task_create(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task creation in database
    fn handle_task_create_db(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task claim
    fn handle_task_claim(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task request
    fn handle_task_request(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task completion
    fn handle_task_complete(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task completion in database
    fn handle_task_complete_db(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task audit
    fn handle_task_audit(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task delegation
    fn handle_task_delegate(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task list
    fn handle_task_list(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle task cancellation
    fn handle_task_cancel(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle agent details
    fn handle_agent_details(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle authentication challenge
    fn handle_auth_challenge(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle authentication verification
    fn handle_auth_verify(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle quick authentication
    fn handle_auth_quick(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle session heartbeat (for authenticated sessions)
    fn handle_session_heartbeat(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle lock acquisition
    fn handle_lock_acquire(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle lock release
    fn handle_lock_release(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle context export
    fn handle_context_export(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle context import
    fn handle_context_import(&self, req: &JsonRpcRequest) -> JsonRpcResponse;

    /// Handle statistics
    fn handle_stats(&self, req: &JsonRpcRequest) -> JsonRpcResponse;
}

/// Common helper functions for RPC handlers
pub mod helpers {
    use serde_json::Value;

    /// Extract arguments from params
    pub fn get_args(params: Option<&Value>) -> Option<&Value> {
        params.and_then(|p| p.get("arguments"))
    }

    /// Extract string argument
    pub fn get_string_arg(params: Option<&Value>, key: &str) -> Option<String> {
        super::get_string_param(params, key)
    }

    /// Extract optional string argument
    pub fn get_optional_string_arg(params: Option<&Value>, key: &str) -> Option<String> {
        super::get_optional_string_param(params, key)
    }

    /// Extract integer argument
    pub fn get_i64_arg(params: Option<&Value>, key: &str) -> Option<i64> {
        super::get_i64_param(params, key)
    }

    /// Extract boolean argument
    pub fn get_bool_arg(params: Option<&Value>, key: &str) -> Option<bool> {
        super::get_bool_param(params, key)
    }

    /// Create error response
    pub fn error_response(
        id: Option<Value>,
        message: &str,
    ) -> crate::core::json_rpc::JsonRpcResponse {
        crate::core::json_rpc::JsonRpcResponse::error(
            id,
            crate::core::json_rpc::error_codes::SERVER_ERROR,
            message,
            None,
        )
    }

    /// Create success response
    pub fn success_response(
        id: Option<Value>,
        result: Value,
    ) -> crate::core::json_rpc::JsonRpcResponse {
        crate::core::json_rpc::JsonRpcResponse::success(id, result)
    }
}
