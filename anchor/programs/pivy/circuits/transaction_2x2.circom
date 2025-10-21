pragma circom 2.0.0;

include "./transaction.circom";

/**
 * PIVY Transaction Circuit Instantiation: 2 inputs, 2 outputs
 *
 * This is the STANDARD circuit for basic operations:
 * - Deposit: Create 1-2 UTXOs (inputs are empty)
 * - Withdraw: Spend 1-2 UTXOs, withdraw to public wallet
 * - Transfer: Consolidate or split UTXOs
 *
 * Tree depth: 26 levels = 67,108,864 capacity
 */
component main {public [
    root,
    publicAmount,
    extDataHash,
    inputNullifier,
    outputCommitment
]} = Transaction(26, 2, 2);
