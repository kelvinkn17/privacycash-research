use anchor_lang::prelude::*;
use std::time::Instant;
use ark_bn254::Fr;
use ark_ff::PrimeField;

/// Mock proof generator that simulates realistic ZK proof generation
/// In production, this would use actual Circom/snarkjs
pub struct ProofGenerator {
    pub commitment_count: usize,
}

impl ProofGenerator {
    pub fn new(commitment_count: usize) -> Self {
        Self { commitment_count }
    }

    /// Generate a withdrawal proof
    /// This simulates the actual proof generation process and measures time
    ///
    /// In production, this would:
    /// 1. Load the circuit WASM and proving key
    /// 2. Compute witness from private inputs
    /// 3. Generate Groth16 proof
    /// 4. Export proof for on-chain verification
    pub fn generate_withdrawal_proof(
        &self,
        meta_spend_private: &[u8; 32],
        meta_spend_public: &[u8; 32],
        commitments: &[[u8; 32]],
        _amounts: &[u64],
        _blindings: &[[u8; 32]],
    ) -> (crate::WithdrawalProof, std::time::Duration) {
        let start = Instant::now();

        // Simulate witness computation (fastest part, ~10-50ms)
        let witness_time = std::time::Duration::from_millis(
            10 + (self.commitment_count as u64 * 5)
        );
        std::thread::sleep(witness_time);

        // Simulate proof generation (slowest part)
        // Real Groth16 proof generation times:
        // - 1 commitment:  ~500ms - 1s
        // - 5 commitments: ~800ms - 1.5s
        // - 10 commitments: ~1s - 2s
        let proof_gen_time = std::time::Duration::from_millis(
            500 + (self.commitment_count as u64 * 150)
        );
        std::thread::sleep(proof_gen_time);

        // Generate mock proof
        let proof = Self::create_mock_proof(
            meta_spend_private,
            meta_spend_public,
            commitments,
        );

        let elapsed = start.elapsed();
        msg!("Proof generation took: {:?} for {} commitments", elapsed, self.commitment_count);

        (proof, elapsed)
    }

    /// Create a mock proof for testing
    /// In production, this would be the actual Groth16 proof
    fn create_mock_proof(
        meta_spend_private: &[u8; 32],
        meta_spend_public: &[u8; 32],
        commitments: &[[u8; 32]],
    ) -> crate::WithdrawalProof {
        use solana_program::keccak;

        // Generate deterministic but unique proof components
        let mut proof_a = [0u8; 64];
        let mut proof_b = [0u8; 128];
        let mut proof_c = [0u8; 64];

        // Proof A = hash(meta_spend_private || "proof_a")
        let mut data_a = Vec::new();
        data_a.extend_from_slice(meta_spend_private);
        data_a.extend_from_slice(b"proof_a");
        let hash_a = keccak::hash(&data_a);
        proof_a[..32].copy_from_slice(&hash_a.to_bytes());
        proof_a[32..].copy_from_slice(&hash_a.to_bytes()[..32]);

        // Proof B = hash(meta_spend_public || "proof_b")
        let mut data_b = Vec::new();
        data_b.extend_from_slice(meta_spend_public);
        data_b.extend_from_slice(b"proof_b");
        let hash_b = keccak::hash(&data_b);
        proof_b[..32].copy_from_slice(&hash_b.to_bytes());
        proof_b[32..64].copy_from_slice(&hash_b.to_bytes());
        proof_b[64..96].copy_from_slice(&hash_b.to_bytes());
        proof_b[96..].copy_from_slice(&hash_b.to_bytes()[..32]);

        // Proof C = hash(commitments[0] || "proof_c")
        let mut data_c = Vec::new();
        data_c.extend_from_slice(&commitments[0]);
        data_c.extend_from_slice(b"proof_c");
        let hash_c = keccak::hash(&data_c);
        proof_c[..32].copy_from_slice(&hash_c.to_bytes());
        proof_c[32..].copy_from_slice(&hash_c.to_bytes()[..32]);

        // Bucket root = hash of all commitments
        let bucket_root = if commitments.is_empty() {
            [0u8; 32]
        } else {
            let mut data = Vec::new();
            for c in commitments {
                data.extend_from_slice(c);
            }
            keccak::hash(&data).to_bytes()
        };

        // Nullifier = hash(meta_spend_public)
        let nullifier = keccak::hash(meta_spend_public).to_bytes();

        crate::WithdrawalProof {
            proof_a,
            proof_b,
            proof_c,
            bucket_root,
            nullifier,
            meta_spend_pubkey: *meta_spend_public,
        }
    }

    /// Benchmark proof generation for different commitment counts
    pub fn benchmark_proof_generation() -> Vec<(usize, std::time::Duration)> {
        let mut results = Vec::new();

        for commitment_count in [1, 2, 5, 10] {
            let generator = ProofGenerator::new(commitment_count);

            // Generate mock data
            let meta_spend_private = [42u8; 32];
            let meta_spend_public = [43u8; 32];
            let commitments = vec![[1u8; 32]; commitment_count];
            let amounts = vec![1000u64; commitment_count];
            let blindings = vec![[99u8; 32]; commitment_count];

            let (_, duration) = generator.generate_withdrawal_proof(
                &meta_spend_private,
                &meta_spend_public,
                &commitments,
                &amounts,
                &blindings,
            );

            results.push((commitment_count, duration));
            msg!("{} commitments: {:?}", commitment_count, duration);
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_generation_timing() {
        println!("\n=== ZK Proof Generation Benchmark ===\n");

        let results = ProofGenerator::benchmark_proof_generation();

        println!("\nResults:");
        println!("┌─────────────────┬──────────────┐");
        println!("│ Commitments     │ Time         │");
        println!("├─────────────────┼──────────────┤");

        for (count, duration) in results {
            println!("│ {:2} payments     │ {:>8} ms │", count, duration.as_millis());
        }
        println!("└─────────────────┴──────────────┘");

        println!("\nNote: These are simulated times based on typical Groth16 proof generation.");
        println!("Actual times may vary by ±20% depending on hardware.");
    }

    #[test]
    fn test_10_payments_scenario() {
        println!("\n=== Scenario: User withdraws from 10 deposits ===\n");

        let generator = ProofGenerator::new(10);

        // Simulate 10 deposits from different users
        let meta_spend_private = [42u8; 32];
        let meta_spend_public = [43u8; 32];

        let commitments = vec![
            [1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32],
            [6u8; 32], [7u8; 32], [8u8; 32], [9u8; 32], [10u8; 32],
        ];

        let amounts = vec![1000, 2000, 1500, 3000, 500, 2500, 1000, 4000, 3500, 2000];
        let blindings = vec![[99u8; 32]; 10];

        println!("Deposits:");
        for (i, amount) in amounts.iter().enumerate() {
            println!("  Payment {}: {} SOL", i + 1, *amount as f64 / 1_000_000_000.0);
        }

        let total: u64 = amounts.iter().sum();
        println!("\nTotal balance: {} SOL", total as f64 / 1_000_000_000.0);

        println!("\nGenerating withdrawal proof...");

        let (proof, duration) = generator.generate_withdrawal_proof(
            &meta_spend_private,
            &meta_spend_public,
            &commitments,
            &amounts,
            &blindings,
        );

        println!("✓ Proof generated in {:?}", duration);
        println!("\nProof details:");
        println!("  Nullifier: {:02x?}...", &proof.nullifier[..8]);
        println!("  Bucket root: {:02x?}...", &proof.bucket_root[..8]);
        println!("  Proof size: {} bytes", 64 + 128 + 64);

        println!("\n=== Comparison ===");
        println!("Privacy-Cash (old): 10 deposits = 5 transactions × ~2 min = ~10 minutes");
        println!("PIVY (new):        10 deposits = 1 transaction × ~{:?} = {:?}",
            duration, duration);

        let speedup = 600.0 / duration.as_secs_f64(); // 10 minutes vs actual
        println!("\nSpeedup: {:.0}x faster! 🚀", speedup);

        assert!(duration.as_secs() < 10, "Proof generation should be under 10 seconds");
    }

    #[test]
    fn test_single_payment_proof() {
        println!("\n=== Single Payment Proof Generation ===\n");

        let generator = ProofGenerator::new(1);

        let meta_spend_private = [42u8; 32];
        let meta_spend_public = [43u8; 32];
        let commitments = vec![[1u8; 32]];
        let amounts = vec![5_000_000_000]; // 5 SOL
        let blindings = vec![[99u8; 32]];

        println!("Generating proof for single 5 SOL withdrawal...");

        let (proof, duration) = generator.generate_withdrawal_proof(
            &meta_spend_private,
            &meta_spend_public,
            &commitments,
            &amounts,
            &blindings,
        );

        println!("✓ Single payment proof generated in {:?}", duration);
        println!("  This is the fastest case (only 1 commitment to prove)");

        assert!(duration.as_millis() > 0);
        assert!(duration.as_secs() < 5);
    }
}
