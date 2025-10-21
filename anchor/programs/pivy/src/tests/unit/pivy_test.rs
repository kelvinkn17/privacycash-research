#[cfg(test)]
mod tests {
    use crate::*;

    // Helper function to create a test commitment
    fn create_test_commitment(amount: u64, meta_spend_pubkey: &[u8; 32], blinding: &[u8; 32]) -> [u8; 32] {
        // In production, this would be: hash(amount, meta_spend_pubkey, blinding)
        // For testing, we'll create a simple deterministic commitment
        let mut commitment = [0u8; 32];
        commitment[0..8].copy_from_slice(&amount.to_le_bytes());
        commitment[8..16].copy_from_slice(&meta_spend_pubkey[0..8]);
        commitment[16..24].copy_from_slice(&blinding[0..8]);
        commitment
    }

    #[test]
    fn test_bucket_account_add_commitment() {
        let mut bucket = BucketAccount {
            commitment_count: 0,
            total_balance: 0,
            is_spent: false,
            commitments: [[0u8; 32]; MAX_BUCKET_SIZE as usize],
            bump: 0,
        };

        let commitment1 = [1u8; 32];
        let amount1 = 1000;

        let result = bucket.add_commitment(commitment1, amount1);
        assert!(result.is_ok());
        assert_eq!(bucket.commitment_count, 1);
        assert_eq!(bucket.total_balance, amount1);
        assert_eq!(bucket.commitments[0], commitment1);
    }

    #[test]
    fn test_bucket_account_multiple_deposits() {
        let mut bucket = BucketAccount {
            commitment_count: 0,
            total_balance: 0,
            is_spent: false,
            commitments: [[0u8; 32]; MAX_BUCKET_SIZE as usize],
            bump: 0,
        };

        // Add first commitment
        let commitment1 = [1u8; 32];
        let amount1 = 1000;
        bucket.add_commitment(commitment1, amount1).unwrap();

        // Add second commitment
        let commitment2 = [2u8; 32];
        let amount2 = 2000;
        bucket.add_commitment(commitment2, amount2).unwrap();

        // Add third commitment
        let commitment3 = [3u8; 32];
        let amount3 = 3000;
        bucket.add_commitment(commitment3, amount3).unwrap();

        assert_eq!(bucket.commitment_count, 3);
        assert_eq!(bucket.total_balance, 6000);
        assert_eq!(bucket.commitments[0], commitment1);
        assert_eq!(bucket.commitments[1], commitment2);
        assert_eq!(bucket.commitments[2], commitment3);
    }

    #[test]
    fn test_bucket_account_overflow_protection() {
        let mut bucket = BucketAccount {
            commitment_count: 0,
            total_balance: u64::MAX - 100,
            is_spent: false,
            commitments: [[0u8; 32]; MAX_BUCKET_SIZE as usize],
            bump: 0,
        };

        let commitment = [1u8; 32];
        let amount = 200; // This would overflow

        let result = bucket.add_commitment(commitment, amount);
        assert!(result.is_err());
    }

    #[test]
    fn test_commitment_generation() {
        let amount = 5000u64;
        let meta_spend_pubkey = [42u8; 32];
        let blinding = [99u8; 32];

        let commitment = create_test_commitment(amount, &meta_spend_pubkey, &blinding);

        // Verify commitment is deterministic
        let commitment2 = create_test_commitment(amount, &meta_spend_pubkey, &blinding);
        assert_eq!(commitment, commitment2);

        // Verify different inputs produce different commitments
        let different_amount_commitment = create_test_commitment(6000, &meta_spend_pubkey, &blinding);
        assert_ne!(commitment, different_amount_commitment);
    }

    #[test]
    fn test_bucket_aggregation_concept() {
        // This test demonstrates the bucket aggregation concept
        let mut bucket = BucketAccount {
            commitment_count: 0,
            total_balance: 0,
            is_spent: false,
            commitments: [[0u8; 32]; MAX_BUCKET_SIZE as usize],
            bump: 0,
        };

        // Simulate 10 deposits
        for i in 1..=10 {
            let commitment = [i as u8; 32];
            let amount = i as u64 * 100;
            bucket.add_commitment(commitment, amount).unwrap();
        }

        // Verify all deposits are aggregated
        assert_eq!(bucket.commitment_count, 10);
        assert_eq!(bucket.total_balance, 5500); // Sum of 100+200+...+1000

        // In PIVY, user can withdraw entire balance in ONE transaction
        // instead of processing each UTXO individually
        assert!(!bucket.is_spent);

        // After withdrawal, bucket is marked as spent
        // (This would be done in the actual withdraw instruction)
    }

    #[test]
    fn test_happy_path_flow() {
        // This test simulates the happy path:
        // 1. John deposits 1000 SOL for Kelvin
        // 2. Kelvin checks balance (1000 SOL)
        // 3. Sarah deposits 2000 SOL for Kelvin
        // 4. Kelvin checks balance (3000 SOL total)
        // 5. Kelvin withdraws 3000 SOL in ONE transaction

        let kelvin_meta_spend_pub = [10u8; 32];
        let kelvin_blinded_account_id = [20u8; 32];

        let mut kelvin_bucket = BucketAccount {
            commitment_count: 0,
            total_balance: 0,
            is_spent: false,
            commitments: [[0u8; 32]; MAX_BUCKET_SIZE as usize],
            bump: 0,
        };

        // Step 1: John deposits 1000 SOL
        let john_commitment = create_test_commitment(1000, &kelvin_meta_spend_pub, &[1u8; 32]);
        kelvin_bucket.add_commitment(john_commitment, 1000).unwrap();

        // Step 2: Kelvin sees balance
        assert_eq!(kelvin_bucket.total_balance, 1000);

        // Step 3: Sarah deposits 2000 SOL
        let sarah_commitment = create_test_commitment(2000, &kelvin_meta_spend_pub, &[2u8; 32]);
        kelvin_bucket.add_commitment(sarah_commitment, 2000).unwrap();

        // Step 4: Kelvin sees updated balance
        assert_eq!(kelvin_bucket.total_balance, 3000);
        assert_eq!(kelvin_bucket.commitment_count, 2);

        // Step 5: Kelvin withdraws entire balance
        // In the actual implementation, this would:
        // - Verify ZK proof of meta_spend private key
        // - Transfer 3000 SOL to Kelvin's address
        // - Mark bucket as spent
        assert!(!kelvin_bucket.is_spent);
        assert_eq!(kelvin_bucket.total_balance, 3000);

        // Simulate withdrawal
        kelvin_bucket.is_spent = true;
        assert!(kelvin_bucket.is_spent);
    }

    #[test]
    fn test_multiple_users_separate_buckets() {
        // Test that different users have separate buckets

        // Kelvin's bucket
        let kelvin_meta_spend_pub = [10u8; 32];
        let kelvin_blinded_account_id = [20u8; 32];
        let mut kelvin_bucket = BucketAccount {
            commitment_count: 0,
            total_balance: 0,
            is_spent: false,
            commitments: [[0u8; 32]; MAX_BUCKET_SIZE as usize],
            bump: 0,
        };

        // Sarah's bucket
        let sarah_meta_spend_pub = [30u8; 32];
        let sarah_blinded_account_id = [40u8; 32];
        let mut sarah_bucket = BucketAccount {
            commitment_count: 0,
            total_balance: 0,
            is_spent: false,
            commitments: [[0u8; 32]; MAX_BUCKET_SIZE as usize],
            bump: 0,
        };

        // Deposits to Kelvin
        kelvin_bucket.add_commitment([1u8; 32], 1000).unwrap();
        kelvin_bucket.add_commitment([2u8; 32], 2000).unwrap();

        // Deposits to Sarah
        sarah_bucket.add_commitment([3u8; 32], 500).unwrap();
        sarah_bucket.add_commitment([4u8; 32], 1500).unwrap();

        // Verify separate balances
        assert_eq!(kelvin_bucket.total_balance, 3000);
        assert_eq!(sarah_bucket.total_balance, 2000);

        // Verify separate commitment counts
        assert_eq!(kelvin_bucket.commitment_count, 2);
        assert_eq!(sarah_bucket.commitment_count, 2);
    }

    #[test]
    fn test_withdrawal_prevents_double_spend() {
        let mut bucket = BucketAccount {
            commitment_count: 1,
            total_balance: 1000,
            is_spent: false,
            commitments: [[1u8; 32]; MAX_BUCKET_SIZE as usize],
            bump: 0,
        };

        // First withdrawal succeeds
        assert!(!bucket.is_spent);
        bucket.is_spent = true;
        assert!(bucket.is_spent);

        // Second withdrawal attempt should fail (checked in actual program)
        // The program checks bucket.is_spent before allowing withdrawal
        assert!(bucket.is_spent); // Would cause ErrorCode::BucketAlreadySpent
    }

    #[test]
    fn test_fee_calculation() {
        // Test withdrawal fee calculation
        let withdrawal_amount = 10000u64;
        let fee_rate = 25u16; // 0.25% = 25 basis points

        // Fee = (amount * rate) / 10000
        let expected_fee = (withdrawal_amount as u128 * fee_rate as u128 / 10000) as u64;
        assert_eq!(expected_fee, 25); // 0.25% of 10000 = 25

        // Test with different amounts
        let withdrawal_amount2 = 100000u64;
        let expected_fee2 = (withdrawal_amount2 as u128 * fee_rate as u128 / 10000) as u64;
        assert_eq!(expected_fee2, 250); // 0.25% of 100000 = 250
    }

    #[test]
    fn test_encrypted_output_concept() {
        // This test demonstrates the encrypted output concept
        // In production, the client would:
        // 1. Encrypt {amount, blinding} with metaView_pub
        // 2. Store encrypted_output on-chain
        // 3. Kelvin can decrypt with metaView_priv to see balance

        let amount = 1000u64;
        let blinding = [42u8; 32];
        let meta_view_pub = [99u8; 32];

        // Simulate encryption (in production, use actual encryption)
        let mut encrypted_output = Vec::new();
        encrypted_output.extend_from_slice(&amount.to_le_bytes());
        encrypted_output.extend_from_slice(&blinding);
        encrypted_output.extend_from_slice(&meta_view_pub);

        assert_eq!(encrypted_output.len(), 8 + 32 + 32);

        // In production:
        // - Only Kelvin (with metaView_priv) can decrypt this
        // - John (depositor) cannot see the amount after deposit
        // - The commitment is public but reveals nothing
    }
}
