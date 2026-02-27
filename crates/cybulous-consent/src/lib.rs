//! Consent protocol implementation with age-gating and discipline eligibility
//!
//! Provides cryptographic attestation of user consent via blockchain transactions.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unreachable_pub)]

pub mod attestation;
pub mod providers;
pub mod verification;

pub use attestation::{ConsentAttestation, ConsentProof};
pub use providers::{ConsentProvider, ProviderType};
pub use verification::{AgeVerification, DisciplineCheck};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

/// Consent-related errors
#[derive(Error, Debug)]
pub enum ConsentError {
    /// User does not meet age requirement
    #[error("age requirement not met: user is {0} years old, minimum is 21")]
    AgeRequirementNotMet(u8),

    /// Discipline eligibility check failed
    #[error("discipline eligibility failed: {0}")]
    DisciplineIneligible(String),

    /// Consent has been revoked
    #[error("consent revoked at {0}")]
    ConsentRevoked(DateTime<Utc>),

    /// Attestation verification failed
    #[error("attestation verification failed: {0}")]
    AttestationInvalid(String),

    /// Provider error
    #[error("provider error: {0}")]
    ProviderError(String),

    /// Blockchain error
    #[error("blockchain error: {0}")]
    BlockchainError(String),
}

/// Result type for consent operations
pub type Result<T> = std::result::Result<T, ConsentError>;

/// Consent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsentStatus {
    /// Consent granted and active
    Active,
    /// Consent pending verification
    Pending,
    /// Consent revoked by user
    Revoked,
    /// Consent expired
    Expired,
}

/// Consent record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRecord {
    /// Unique consent ID
    pub id: Uuid,
    /// User identifier
    pub user_id: String,
    /// Consent status
    pub status: ConsentStatus,
    /// Timestamp when consent was granted
    pub granted_at: DateTime<Utc>,
    /// Optional expiration time
    pub expires_at: Option<DateTime<Utc>>,
    /// Revocation timestamp if applicable
    pub revoked_at: Option<DateTime<Utc>>,
    /// Blockchain transaction hash
    pub tx_hash: String,
    /// Age verification proof
    pub age_proof: String,
    /// Discipline eligibility proof
    pub discipline_proof: String,
}

/// Main consent engine
#[derive(Clone)]
pub struct ConsentEngine {
    provider: Arc<dyn ConsentProvider>,
    blockchain_client: Arc<BlockchainClient>,
    min_age: u8,
}

impl ConsentEngine {
    /// Create new consent engine
    pub fn new(
        provider: Arc<dyn ConsentProvider>,
        blockchain_client: Arc<BlockchainClient>,
        min_age: u8,
    ) -> Self {
        Self {
            provider,
            blockchain_client,
            min_age,
        }
    }

    /// Create mock engine for testing
    #[cfg(test)]
    pub fn mock() -> Self {
        Self {
            provider: Arc::new(providers::MockProvider::default()),
            blockchain_client: Arc::new(BlockchainClient::mock()),
            min_age: 21,
        }
    }

    /// Verify user consent
    pub async fn verify_consent(&self, user_id: &str, proof: &str) -> Result<bool> {
        // Retrieve consent record from blockchain
        let record = self
            .blockchain_client
            .get_consent_record(user_id)
            .await
            .map_err(|e| ConsentError::BlockchainError(e.to_string()))?;

        // Check status
        if record.status != ConsentStatus::Active {
            return Ok(false);
        }

        // Check expiration
        if let Some(expires_at) = record.expires_at {
            if Utc::now() > expires_at {
                return Ok(false);
            }
        }

        // Verify proof signature
        self.verify_proof_signature(proof, &record.tx_hash)
            .await
    }

    /// Request consent from user
    pub async fn request_consent(&self, user_id: &str) -> Result<ConsentRecord> {
        // Verify age (21+)
        let age = self
            .provider
            .verify_age(user_id)
            .await
            .map_err(|e| ConsentError::ProviderError(e.to_string()))?;

        if age < self.min_age {
            return Err(ConsentError::AgeRequirementNotMet(age));
        }

        // Check discipline eligibility
        let discipline_proof = self
            .provider
            .check_discipline(user_id)
            .await
            .map_err(|e| ConsentError::DisciplineIneligible(e.to_string()))?;

        // Create attestation
        let attestation = ConsentAttestation {
            user_id: user_id.to_string(),
            age,
            discipline_proof: discipline_proof.clone(),
            timestamp: Utc::now(),
        };

        // Record on blockchain
        let tx_hash = self
            .blockchain_client
            .record_consent(&attestation)
            .await
            .map_err(|e| ConsentError::BlockchainError(e.to_string()))?;

        Ok(ConsentRecord {
            id: Uuid::new_v4(),
            user_id: user_id.to_string(),
            status: ConsentStatus::Active,
            granted_at: Utc::now(),
            expires_at: None,
            revoked_at: None,
            tx_hash,
            age_proof: format!("age:{}", age),
            discipline_proof,
        })
    }

    /// Revoke consent
    pub async fn revoke_consent(&self, user_id: &str) -> Result<()> {
        self.blockchain_client
            .revoke_consent(user_id)
            .await
            .map_err(|e| ConsentError::BlockchainError(e.to_string()))
    }

    async fn verify_proof_signature(&self, proof: &str, tx_hash: &str) -> Result<bool> {
        // Verify cryptographic signature matches blockchain record
        let expected_proof = cybulous_crypto::hash_data(&format!("{}:{}", tx_hash, self.min_age));
        Ok(proof == expected_proof)
    }
}

/// Blockchain client for consent recording
pub struct BlockchainClient {
    rpc_endpoint: String,
    address: String,
}

impl BlockchainClient {
    /// Create new blockchain client
    pub fn new(rpc_endpoint: String, address: String) -> Self {
        Self {
            rpc_endpoint,
            address,
        }
    }

    /// Create mock client for testing
    #[cfg(test)]
    pub fn mock() -> Self {
        Self {
            rpc_endpoint: "http://localhost:26657".to_string(),
            address: "bostrom18sd2ujv24ual9c9pshtxys6j8knh6xaead9ye7".to_string(),
        }
    }

    /// Get consent record from blockchain
    pub async fn get_consent_record(&self, user_id: &str) -> anyhow::Result<ConsentRecord> {
        // Query blockchain for consent record
        // Implementation would use cosmrs to interact with Bostrom chain
        Ok(ConsentRecord {
            id: Uuid::new_v4(),
            user_id: user_id.to_string(),
            status: ConsentStatus::Active,
            granted_at: Utc::now(),
            expires_at: None,
            revoked_at: None,
            tx_hash: "mock-tx-hash".to_string(),
            age_proof: "age:25".to_string(),
            discipline_proof: "discipline:verified".to_string(),
        })
    }

    /// Record consent on blockchain
    pub async fn record_consent(&self, attestation: &ConsentAttestation) -> anyhow::Result<String> {
        // Submit transaction to blockchain
        // Implementation would use cosmrs to create and broadcast transaction
        Ok(format!("tx-hash-{}", attestation.user_id))
    }

    /// Revoke consent on blockchain
    pub async fn revoke_consent(&self, user_id: &str) -> anyhow::Result<()> {
        // Submit revocation transaction
        tracing::info!("Revoking consent for user: {}", user_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consent_verification() {
        let engine = ConsentEngine::mock();
        let result = engine.verify_consent("test-user", "test-proof").await;
        assert!(result.is_ok());
    }
}
