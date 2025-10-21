pragma circom 2.0.0;

include "./bucket_withdrawal_partial.circom";

/**
 * PIVY Bucket Withdrawal - 50 Deposits with Partial Withdrawal Support
 *
 * Larger bucket for power users:
 * - Up to 50 deposits in one transaction
 * - Full partial withdrawal support
 * - Proof time: ~40-50 seconds
 * - Memory: ~4-5GB RAM
 *
 * Perfect for:
 * - Merchants receiving many payments
 * - Services with frequent deposits
 * - Users who don't withdraw often
 *
 * Example:
 * - Receive 50 deposits × 2 SOL = 100 SOL
 * - Withdraw 75 SOL: Select 38 deposits (76 SOL), get 75 SOL + 1 SOL change
 * - Later withdraw 20 SOL: Select 10 deposits + use change UTXO
 */
component main {public [
    root,
    metaSpendPublic,
    withdrawalAmount,
    extDataHash,
    mintAddress,
    outputNullifiers,
    outputChangeCommitment
]} = BucketWithdrawalPartial(26, 50);
