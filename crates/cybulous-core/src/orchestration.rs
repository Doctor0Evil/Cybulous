//! Orchestration layer for multi-agent coordination and tool invocation
//!
//! Implements deterministic execution with consent-gated access control.

use crate::{CybulousError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Tool invocation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for the tool call
    pub id: Uuid,
    /// Name of the tool to invoke
    pub tool_name: String,
    /// Input parameters for the tool
    pub parameters: serde_json::Value,
    /// User ID making the request
    pub user_id: String,
    /// Execution context
    pub context: ExecutionContext,
    /// Timeout in milliseconds
    pub timeout_ms: u64,
}

/// Tool execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Session identifier
    pub session_id: Uuid,
    /// Consent attestation proof
    pub consent_proof: String,
    /// Biophysical signature hash
    pub biophysical_hash: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Tool execution response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// Call ID this response corresponds to
    pub call_id: Uuid,
    /// Execution status
    pub status: ExecutionStatus,
    /// Response payload
    pub result: Option<serde_json::Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution succeeded
    Success,
    /// Execution failed
    Failed,
    /// Execution timed out
    Timeout,
    /// Consent not granted
    ConsentDenied,
}

/// Tool executor trait
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool call
    async fn execute(&self, call: &ToolCall) -> Result<ToolResponse>;

    /// Get tool name
    fn name(&self) -> &str;

    /// Check if tool supports given capability
    fn supports_capability(&self, capability: &str) -> bool;
}

/// Orchestrator for managing tool executions
#[derive(Clone)]
pub struct Orchestrator {
    executors: Arc<RwLock<HashMap<String, Arc<dyn ToolExecutor>>>>,
    consent_engine: Arc<cybulous_consent::ConsentEngine>,
    max_concurrent: usize,
}

impl Orchestrator {
    /// Create a new orchestrator instance
    pub fn new(
        consent_engine: Arc<cybulous_consent::ConsentEngine>,
        max_concurrent: usize,
    ) -> Self {
        Self {
            executors: Arc::new(RwLock::new(HashMap::new())),
            consent_engine,
            max_concurrent,
        }
    }

    /// Register a tool executor
    pub async fn register_executor(&self, executor: Arc<dyn ToolExecutor>) -> Result<()> {
        let name = executor.name().to_string();
        let mut executors = self.executors.write().await;

        if executors.contains_key(&name) {
            warn!("Overwriting existing executor: {}", name);
        }

        executors.insert(name.clone(), executor);
        info!("Registered executor: {}", name);
        Ok(())
    }

    /// Execute a tool call with consent verification
    pub async fn execute_tool(&self, call: ToolCall) -> Result<ToolResponse> {
        let start = std::time::Instant::now();

        // Verify consent before execution
        self.verify_consent(&call).await?;

        // Find executor
        let executors = self.executors.read().await;
        let executor = executors.get(&call.tool_name).ok_or_else(|| {
            CybulousError::OrchestrationFailed(format!("Unknown tool: {}", call.tool_name))
        })?;

        // Execute with timeout
        let timeout = tokio::time::Duration::from_millis(call.timeout_ms);
        let execution = executor.execute(&call);

        match tokio::time::timeout(timeout, execution).await {
            Ok(Ok(mut response)) => {
                response.duration_ms = start.elapsed().as_millis() as u64;
                info!(
                    "Tool {} executed successfully in {}ms",
                    call.tool_name, response.duration_ms
                );
                Ok(response)
            }
            Ok(Err(e)) => {
                error!("Tool {} execution failed: {}", call.tool_name, e);
                Ok(ToolResponse {
                    call_id: call.id,
                    status: ExecutionStatus::Failed,
                    result: None,
                    error: Some(e.to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                })
            }
            Err(_) => {
                warn!("Tool {} execution timed out", call.tool_name);
                Ok(ToolResponse {
                    call_id: call.id,
                    status: ExecutionStatus::Timeout,
                    result: None,
                    error: Some("Execution timeout".to_string()),
                    duration_ms: call.timeout_ms,
                })
            }
        }
    }

    /// Verify user consent for tool execution
    async fn verify_consent(&self, call: &ToolCall) -> Result<()> {
        match self
            .consent_engine
            .verify_consent(&call.user_id, &call.context.consent_proof)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(CybulousError::ConsentError(
                "Consent verification failed".to_string(),
            )),
            Err(e) => Err(CybulousError::ConsentError(format!(
                "Consent check error: {}",
                e
            ))),
        }
    }

    /// List all registered tools
    pub async fn list_tools(&self) -> Vec<String> {
        let executors = self.executors.read().await;
        executors.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExecutor {
        name: String,
    }

    #[async_trait]
    impl ToolExecutor for MockExecutor {
        async fn execute(&self, call: &ToolCall) -> Result<ToolResponse> {
            Ok(ToolResponse {
                call_id: call.id,
                status: ExecutionStatus::Success,
                result: Some(serde_json::json!({"executed": true})),
                error: None,
                duration_ms: 10,
            })
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn supports_capability(&self, _capability: &str) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn test_executor_registration() {
        let consent_engine = Arc::new(cybulous_consent::ConsentEngine::mock());
        let orchestrator = Orchestrator::new(consent_engine, 10);

        let executor = Arc::new(MockExecutor {
            name: "test-tool".to_string(),
        });

        orchestrator.register_executor(executor).await.unwrap();

        let tools = orchestrator.list_tools().await;
        assert!(tools.contains(&"test-tool".to_string()));
    }
}
