pragma circom 2.0.0;

include "../../../node_modules/circomlib/circuits/poseidon.circom";
include "../../../node_modules/circomlib/circuits/comparators.circom";
include "../../../node_modules/circomlib/circuits/bitify.circom";
include "./merkleProof.circom";
include "./keypair.circom";

/**
 * PIVY Bucket Withdrawal - WITH PARTIAL WITHDRAWALS!
 *
 * THIS IS THE CORRECT DESIGN FOR PIVY!
 *
 * Key features:
 * - Withdraw from multiple deposits in ONE transaction
 * - Support PARTIAL withdrawals (withdraw less than total)
 * - Create change UTXO for remaining amount
 * - Each deposit gets its own nullifier (prevents double-spend)
 *
 * Example:
 * - You have 10 deposits × 10 SOL = 100 SOL
 * - Withdraw 30 SOL: Select 3 deposits, get 30 SOL out
 * - Withdraw 50 SOL: Select 5 deposits, get 50 SOL out
 * - Withdraw 25 SOL: Select 3 deposits (30 SOL), get 25 SOL out + 5 SOL change UTXO
 */
template BucketWithdrawalPartial(levels, maxDeposits) {
    // ==================== PUBLIC INPUTS ====================
    signal input root;                              // Merkle tree root
    signal input metaSpendPublic;                   // MetaSpend public key
    signal input withdrawalAmount;                  // Amount to withdraw (can be less than total!)
    signal input extDataHash;                       // Hash of recipient, fee, etc
    signal input mintAddress;                       // Token mint

    // Public outputs - one nullifier per deposit spent
    signal output outputNullifiers[maxDeposits];    // One nullifier per deposit
    signal output outputChangeCommitment;           // Change UTXO commitment (if partial)

    // ==================== PRIVATE INPUTS ====================
    signal input metaSpendPrivate;                  // MetaSpend private key
    signal input depositCount;                      // Number of deposits to spend
    signal input amounts[maxDeposits];              // Amount for each deposit
    signal input blindings[maxDeposits];            // Blinding for each deposit
    signal input pathIndices[maxDeposits];          // Merkle path for each
    signal input pathElements[maxDeposits][levels]; // Merkle proof for each

    // For change UTXO
    signal input changeAmount;                      // Amount for change UTXO
    signal input changeBlinding;                    // Blinding for change UTXO

    // ==================== 1. VERIFY KEYPAIR ====================
    component keypair = Keypair();
    keypair.privateKey <== metaSpendPrivate;
    keypair.publicKey === metaSpendPublic;

    // ==================== 2. PROCESS EACH DEPOSIT ====================
    component commitmentHashers[maxDeposits];
    component merkleProofs[maxDeposits];
    component nullifierSignatures[maxDeposits];
    component nullifierHashers[maxDeposits];
    component amountChecks[maxDeposits];

    signal commitments[maxDeposits];
    signal runningTotal[maxDeposits + 1];
    runningTotal[0] <== 0;

    for (var i = 0; i < maxDeposits; i++) {
        // Compute commitment
        commitmentHashers[i] = Poseidon(4);
        commitmentHashers[i].inputs[0] <== amounts[i];
        commitmentHashers[i].inputs[1] <== metaSpendPublic;
        commitmentHashers[i].inputs[2] <== blindings[i];
        commitmentHashers[i].inputs[3] <== mintAddress;
        commitments[i] <== commitmentHashers[i].out;

        // Verify in Merkle tree
        merkleProofs[i] = MerkleProof(levels);
        merkleProofs[i].leaf <== commitments[i];
        merkleProofs[i].pathIndices <== pathIndices[i];
        for (var j = 0; j < levels; j++) {
            merkleProofs[i].pathElements[j] <== pathElements[i][j];
        }

        // Check root (only for active deposits)
        component checkRoot = ForceEqualIfEnabled();
        checkRoot.in[0] <== root;
        checkRoot.in[1] <== merkleProofs[i].root;
        checkRoot.enabled <== amounts[i];

        // Generate signature for nullifier
        nullifierSignatures[i] = Signature();
        nullifierSignatures[i].privateKey <== metaSpendPrivate;
        nullifierSignatures[i].commitment <== commitments[i];
        nullifierSignatures[i].merklePath <== pathIndices[i];

        // Compute nullifier for THIS specific deposit
        // This is what prevents double-spend!
        nullifierHashers[i] = Poseidon(3);
        nullifierHashers[i].inputs[0] <== commitments[i];
        nullifierHashers[i].inputs[1] <== pathIndices[i];
        nullifierHashers[i].inputs[2] <== nullifierSignatures[i].out;
        outputNullifiers[i] <== nullifierHashers[i].out;

        // Range check
        amountChecks[i] = Num2Bits(248);
        amountChecks[i].in <== amounts[i];

        // Accumulate total (only if i < depositCount)
        component isActive = LessThan(8);
        isActive.in[0] <== i;
        isActive.in[1] <== depositCount;
        signal amountToAdd <== amounts[i] * isActive.out;
        runningTotal[i + 1] <== runningTotal[i] + amountToAdd;
    }

    signal totalDeposits <== runningTotal[maxDeposits];

    // ==================== 3. COMPUTE CHANGE ====================
    // changeAmount = totalDeposits - withdrawalAmount
    // If changeAmount > 0, we create a change UTXO
    // If changeAmount == 0, full withdrawal (no change)

    signal computedChange <== totalDeposits - withdrawalAmount;
    computedChange === changeAmount;

    // Compute change commitment
    component changeCommitmentHasher = Poseidon(4);
    changeCommitmentHasher.inputs[0] <== changeAmount;
    changeCommitmentHasher.inputs[1] <== metaSpendPublic;  // Same key!
    changeCommitmentHasher.inputs[2] <== changeBlinding;
    changeCommitmentHasher.inputs[3] <== mintAddress;
    outputChangeCommitment <== changeCommitmentHasher.out;

    // ==================== 4. VERIFY WITHDRAWAL AMOUNT ====================
    // withdrawalAmount must be <= totalDeposits
    component withdrawalCheck = LessEqThan(248);
    withdrawalCheck.in[0] <== withdrawalAmount;
    withdrawalCheck.in[1] <== totalDeposits;
    withdrawalCheck.out === 1;

    // Range check withdrawal amount
    component withdrawalRangeCheck = Num2Bits(248);
    withdrawalRangeCheck.in <== withdrawalAmount;

    // Range check change amount
    component changeRangeCheck = Num2Bits(248);
    changeRangeCheck.in <== changeAmount;

    // ==================== 5. SAFETY CONSTRAINTS ====================
    signal extDataSquare <== extDataHash * extDataHash;

    // Ensure depositCount is reasonable
    component depositCountCheck = LessThan(8);
    depositCountCheck.in[0] <== depositCount;
    depositCountCheck.in[1] <== maxDeposits + 1;
    depositCountCheck.out === 1;
}

template ForceEqualIfEnabled() {
    signal input enabled;
    signal input in[2];

    component isZ = IsZero();
    isZ.in <== enabled;

    component eq = IsEqual();
    eq.in[0] <== in[0];
    eq.in[1] <== in[1];

    (1 - isZ.out) * (1 - eq.out) === 0;
}

template Signature() {
    signal input privateKey;
    signal input commitment;
    signal input merklePath;
    signal output out;

    component hasher = Poseidon(3);
    hasher.inputs[0] <== privateKey;
    hasher.inputs[1] <== commitment;
    hasher.inputs[2] <== merklePath;
    out <== hasher.out;
}

/**
 * Instantiation: 20 deposits
 *
 * Perfect for most use cases:
 * - Proof time: ~20-25 seconds
 * - Memory: ~3GB
 * - Supports partial withdrawals!
 *
 * Example usage:
 * - Receive 10 × 10 SOL deposits (100 SOL total)
 * - Withdraw 30 SOL: Select 3 deposits
 * - Withdraw 50 SOL: Select 5 deposits
 * - Withdraw 25 SOL: Select 3 deposits (30 SOL), get 5 SOL change
 */
component main {public [
    root,
    metaSpendPublic,
    withdrawalAmount,
    extDataHash,
    mintAddress,
    outputNullifiers,
    outputChangeCommitment
]} = BucketWithdrawalPartial(26, 20);
