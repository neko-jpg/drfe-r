//! Sybil Attack Protection for DRFE-R
//!
//! Prevents single entities from creating multiple fake nodes
//! using Proof-of-Work and trust score mechanisms.

use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::RwLock;

use crate::coordinates::NodeId;

/// Proof-of-Work based node ID generator
pub struct ProofOfWork {
    /// Required number of leading zero bits
    pub difficulty: u32,
    /// Expected computation time in milliseconds (approximate)
    pub target_time_ms: u64,
}

impl ProofOfWork {
    pub fn new(difficulty: u32) -> Self {
        Self {
            difficulty,
            target_time_ms: (1 << difficulty) as u64, // Rough estimate
        }
    }

    /// Generate a valid node ID with proof-of-work
    pub fn generate_node_id(&self, data: &[u8]) -> ProofResult {
        let start = std::time::Instant::now();
        let mut nonce: u64 = 0;
        
        loop {
            let hash = Self::compute_hash(data, nonce);
            if self.check_difficulty(&hash) {
                return ProofResult {
                    node_id: NodeId::new(&hex::encode(&hash[..16])),
                    nonce,
                    hash: hex::encode(&hash),
                    attempts: nonce,
                    time_ms: start.elapsed().as_millis() as u64,
                };
            }
            nonce += 1;
            
            // Safety limit: 100 million attempts
            if nonce > 100_000_000 {
                break;
            }
        }
        
        // Fallback (should not happen with reasonable difficulty)
        let hash = Self::compute_hash(data, nonce);
        ProofResult {
            node_id: NodeId::new(&hex::encode(&hash[..16])),
            nonce,
            hash: hex::encode(&hash),
            attempts: nonce,
            time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Verify a proof-of-work
    pub fn verify(&self, data: &[u8], nonce: u64) -> bool {
        let hash = Self::compute_hash(data, nonce);
        self.check_difficulty(&hash)
    }

    fn compute_hash(data: &[u8], nonce: u64) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.update(&nonce.to_le_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    fn check_difficulty(&self, hash: &[u8; 32]) -> bool {
        let mut leading_zeros = 0;
        for byte in hash.iter() {
            if *byte == 0 {
                leading_zeros += 8;
            } else {
                leading_zeros += byte.leading_zeros();
                break;
            }
        }
        leading_zeros >= self.difficulty
    }
}

impl Default for ProofOfWork {
    fn default() -> Self {
        Self::new(8) // ~256 attempts, fast for development
    }
}

/// Result of proof-of-work generation
#[derive(Debug, Clone)]
pub struct ProofResult {
    pub node_id: NodeId,
    pub nonce: u64,
    pub hash: String,
    pub attempts: u64,
    pub time_ms: u64,
}

/// Trust score manager
pub struct TrustManager {
    /// Trust scores (0.0 to 1.0)
    scores: RwLock<HashMap<NodeId, TrustScore>>,
    /// Initial trust for new nodes
    pub initial_trust: f64,
    /// Decay factor per time period
    pub decay_rate: f64,
    /// Reward for good behavior
    pub reward_rate: f64,
    /// Penalty for bad behavior
    pub penalty_rate: f64,
}

/// Trust score with history
#[derive(Debug, Clone)]
pub struct TrustScore {
    pub value: f64,
    pub successful_routes: u64,
    pub failed_routes: u64,
    pub coordinate_violations: u64,
    pub last_update_ns: u64,
}

impl TrustScore {
    fn new(initial: f64) -> Self {
        Self {
            value: initial,
            successful_routes: 0,
            failed_routes: 0,
            coordinate_violations: 0,
            last_update_ns: Self::now_ns(),
        }
    }

    fn now_ns() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

impl TrustManager {
    pub fn new() -> Self {
        Self {
            scores: RwLock::new(HashMap::new()),
            initial_trust: 0.5,
            decay_rate: 0.01,
            reward_rate: 0.05,
            penalty_rate: 0.1,
        }
    }

    /// Get trust score for a node
    pub fn get_trust(&self, node_id: &NodeId) -> f64 {
        self.scores.read().unwrap()
            .get(node_id)
            .map(|s| s.value)
            .unwrap_or(self.initial_trust)
    }

    /// Record successful route through a node
    pub fn record_success(&self, node_id: &NodeId) {
        let mut scores = self.scores.write().unwrap();
        let score = scores.entry(node_id.clone())
            .or_insert_with(|| TrustScore::new(self.initial_trust));
        
        score.successful_routes += 1;
        score.value = (score.value + self.reward_rate).min(1.0);
        score.last_update_ns = TrustScore::now_ns();
    }

    /// Record failed route through a node
    pub fn record_failure(&self, node_id: &NodeId) {
        let mut scores = self.scores.write().unwrap();
        let score = scores.entry(node_id.clone())
            .or_insert_with(|| TrustScore::new(self.initial_trust));
        
        score.failed_routes += 1;
        score.value = (score.value - self.penalty_rate).max(0.0);
        score.last_update_ns = TrustScore::now_ns();
    }

    /// Record coordinate violation
    pub fn record_violation(&self, node_id: &NodeId) {
        let mut scores = self.scores.write().unwrap();
        let score = scores.entry(node_id.clone())
            .or_insert_with(|| TrustScore::new(self.initial_trust));
        
        score.coordinate_violations += 1;
        score.value = (score.value - self.penalty_rate * 2.0).max(0.0);
        score.last_update_ns = TrustScore::now_ns();
    }

    /// Get full trust data
    pub fn get_trust_data(&self, node_id: &NodeId) -> Option<TrustScore> {
        self.scores.read().unwrap().get(node_id).cloned()
    }

    /// Check if node is trusted (score > threshold)
    pub fn is_trusted(&self, node_id: &NodeId, threshold: f64) -> bool {
        self.get_trust(node_id) >= threshold
    }

    /// Get all nodes sorted by trust
    pub fn get_ranking(&self) -> Vec<(NodeId, f64)> {
        let scores = self.scores.read().unwrap();
        let mut ranking: Vec<_> = scores.iter()
            .map(|(id, s)| (id.clone(), s.value))
            .collect();
        ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranking
    }
}

impl Default for TrustManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Node registration with Sybil protection
pub struct SybilProtectedRegistry {
    /// Proof-of-work requirements
    pub pow: ProofOfWork,
    /// Trust manager
    pub trust: TrustManager,
    /// Registered nodes with their proofs
    registered: RwLock<HashMap<NodeId, RegistrationInfo>>,
    /// IP rate limiting
    ip_limits: RwLock<HashMap<String, RateLimitState>>,
    /// Max registrations per IP
    pub max_per_ip: usize,
}

#[derive(Debug, Clone)]
pub struct RegistrationInfo {
    pub node_id: NodeId,
    pub nonce: u64,
    pub ip_address: Option<String>,
    pub registered_at_ns: u64,
}

#[derive(Debug, Clone)]
struct RateLimitState {
    count: usize,
    first_registration_ns: u64,
}

impl SybilProtectedRegistry {
    pub fn new(difficulty: u32) -> Self {
        Self {
            pow: ProofOfWork::new(difficulty),
            trust: TrustManager::new(),
            registered: RwLock::new(HashMap::new()),
            ip_limits: RwLock::new(HashMap::new()),
            max_per_ip: 10,
        }
    }

    /// Register a new node with proof-of-work
    pub fn register(&self, data: &[u8], ip_address: Option<&str>) -> RegistrationResult {
        // Check IP rate limit
        if let Some(ip) = ip_address {
            let mut limits = self.ip_limits.write().unwrap();
            let state = limits.entry(ip.to_string())
                .or_insert_with(|| RateLimitState {
                    count: 0,
                    first_registration_ns: TrustScore::now_ns(),
                });
            
            if state.count >= self.max_per_ip {
                return RegistrationResult::RateLimited {
                    ip: ip.to_string(),
                    max: self.max_per_ip,
                };
            }
            state.count += 1;
        }

        // Generate proof-of-work
        let proof = self.pow.generate_node_id(data);
        
        // Register the node
        let info = RegistrationInfo {
            node_id: proof.node_id.clone(),
            nonce: proof.nonce,
            ip_address: ip_address.map(String::from),
            registered_at_ns: TrustScore::now_ns(),
        };
        
        self.registered.write().unwrap()
            .insert(proof.node_id.clone(), info);
        
        RegistrationResult::Success {
            node_id: proof.node_id,
            nonce: proof.nonce,
            time_ms: proof.time_ms,
        }
    }

    /// Verify a node's registration
    pub fn verify_registration(&self, node_id: &NodeId, data: &[u8], nonce: u64) -> bool {
        if !self.pow.verify(data, nonce) {
            return false;
        }
        
        let registered = self.registered.read().unwrap();
        if let Some(info) = registered.get(node_id) {
            info.nonce == nonce
        } else {
            false
        }
    }

    /// Get registration info
    pub fn get_info(&self, node_id: &NodeId) -> Option<RegistrationInfo> {
        self.registered.read().unwrap().get(node_id).cloned()
    }

    /// Total registered nodes
    pub fn count(&self) -> usize {
        self.registered.read().unwrap().len()
    }
}

impl Default for SybilProtectedRegistry {
    fn default() -> Self {
        Self::new(8)
    }
}

/// Result of node registration
#[derive(Debug, Clone)]
pub enum RegistrationResult {
    Success {
        node_id: NodeId,
        nonce: u64,
        time_ms: u64,
    },
    RateLimited {
        ip: String,
        max: usize,
    },
    InvalidProof,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_of_work() {
        let pow = ProofOfWork::new(4); // Low difficulty for testing
        let data = b"test_node_data";
        
        let result = pow.generate_node_id(data);
        assert!(pow.verify(data, result.nonce));
    }

    #[test]
    fn test_trust_manager() {
        let tm = TrustManager::new();
        let node = NodeId::new("test");
        
        // Initial trust
        assert!((tm.get_trust(&node) - 0.5).abs() < 0.01);
        
        // Success increases trust
        tm.record_success(&node);
        assert!(tm.get_trust(&node) > 0.5);
        
        // Failure decreases trust
        for _ in 0..5 {
            tm.record_failure(&node);
        }
        assert!(tm.get_trust(&node) < 0.5);
    }

    #[test]
    fn test_registry() {
        let registry = SybilProtectedRegistry::new(4);
        let data = b"registration_data";
        
        let result = registry.register(data, Some("127.0.0.1"));
        
        if let RegistrationResult::Success { node_id, nonce, .. } = result {
            assert!(registry.verify_registration(&node_id, data, nonce));
        } else {
            panic!("Registration should succeed");
        }
    }
}

// Hex encoding helper
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
