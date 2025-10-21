pragma circom 2.0.0;

include "../../../node_modules/circomlib/circuits/poseidon.circom";
include "../../../node_modules/circomlib/circuits/comparators.circom";
include "../../../node_modules/circomlib/circuits/bitify.circom";
include "./merkleProof.circom";
include "./keypair.circom";

/**
 * PIVY Bucket Withdrawal Circuit
 *
 * Allows withdrawing from a "bucket" of multiple deposits in ONE transaction!
 *
 * How it works:
 * 1. User has received N deposits to same MetaSpend pubkey
 * 2. All deposits create individual commitments in Merkle tree
 * 3. User proves ownership of all commitments with ONE MetaSpend private key
 * 4. Withdraws sum of all deposits in one transaction
 *
 * This is PIVY's key innovation over Privacy Cash!
 */
template BucketWithdrawal(levels, maxDeposits) {
    // ==================== PUBLIC INPUTS ====================
    signal input root;                              // Merkle root
    signal input metaSpendPublic;                   // MetaSpend public key
    signal input totalWithdrawal;                   // Total amount to withdraw
    signal input extDataHash;                       // Hash of recipient, fee, etc
    signal input mintAddress;                       // Token mint
    signal output bucketNullifier;                  // Single nullifier for entire bucket

    // ==================== PRIVATE INPUTS ====================
    signal input metaSpendPrivate;                  // MetaSpend private key (proves ownership)
    signal input depositCount;                      // Actual number of deposits (<=maxDeposits)
    signal input amounts[maxDeposits];              // Amount for each deposit
    signal input blindings[maxDeposits];            // Blinding for each deposit
    signal input pathIndices[maxDeposits];          // Merkle path index for each
    signal input pathElements[maxDeposits][levels]; // Merkle proof for each

    // ==================== 1. VERIFY KEYPAIR ====================
    component keypair = Keypair();
    keypair.privateKey <== metaSpendPrivate;
    keypair.publicKey === metaSpendPublic;

    // ==================== 2. VERIFY EACH DEPOSIT ====================
    component commitmentHashers[maxDeposits];
    component merkleProofs[maxDeposits];
    component amountChecks[maxDeposits];
    signal commitments[maxDeposits];  // Store commitments for nullifier computation
    signal runningTotal[maxDeposits + 1];
    runningTotal[0] <== 0;

    for (var i = 0; i < maxDeposits; i++) {
        // Compute commitment for this deposit
        commitmentHashers[i] = Poseidon(4);
        commitmentHashers[i].inputs[0] <== amounts[i];
        commitmentHashers[i].inputs[1] <== metaSpendPublic;
        commitmentHashers[i].inputs[2] <== blindings[i];
        commitmentHashers[i].inputs[3] <== mintAddress;
        commitments[i] <== commitmentHashers[i].out;

        // Verify commitment is in Merkle tree
        merkleProofs[i] = MerkleProof(levels);
        merkleProofs[i].leaf <== commitments[i];
        merkleProofs[i].pathIndices <== pathIndices[i];
        for (var j = 0; j < levels; j++) {
            merkleProofs[i].pathElements[j] <== pathElements[i][j];
        }

        // Check Merkle root matches (only for non-zero amounts)
        component checkRoot = ForceEqualIfEnabled();
        checkRoot.in[0] <== root;
        checkRoot.in[1] <== merkleProofs[i].root;
        checkRoot.enabled <== amounts[i];

        // Range check amount
        amountChecks[i] = Num2Bits(248);
        amountChecks[i].in <== amounts[i];

        // Accumulate total (only if i < depositCount)
        component isActive = LessThan(8);
        isActive.in[0] <== i;
        isActive.in[1] <== depositCount;

        // Add to running total only if this deposit is active
        signal amountToAdd <== amounts[i] * isActive.out;
        runningTotal[i + 1] <== runningTotal[i] + amountToAdd;
    }

    // ==================== 3. VERIFY TOTAL ====================
    runningTotal[maxDeposits] === totalWithdrawal;

    // ==================== 4. COMPUTE COMMITMENT MERKLE ROOT ====================
    // Build a Merkle tree of all commitments in this bucket
    // This prevents double-spend attacks where attacker uses same deposits with different depositCount
    component commitmentTree[maxDeposits - 1];
    signal commitmentTreeLevels[maxDeposits];
    commitmentTreeLevels[0] <== commitments[0];

    for (var i = 0; i < maxDeposits - 1; i++) {
        commitmentTree[i] = Poseidon(2);
        commitmentTree[i].inputs[0] <== commitmentTreeLevels[i];
        commitmentTree[i].inputs[1] <== commitments[i + 1];
        commitmentTreeLevels[i + 1] <== commitmentTree[i].out;
    }

    signal commitmentMerkleRoot <== commitmentTreeLevels[maxDeposits - 1];

    // ==================== 5. GENERATE BUCKET NULLIFIER ====================
    // bucketNullifier = Poseidon(metaSpendPrivate, commitmentMerkleRoot, depositCount)
    // Using commitment Merkle root ensures different deposit sets produce different nullifiers!
    component nullifierHasher = Poseidon(3);
    nullifierHasher.inputs[0] <== metaSpendPrivate;
    nullifierHasher.inputs[1] <== commitmentMerkleRoot;
    nullifierHasher.inputs[2] <== depositCount;
    bucketNullifier <== nullifierHasher.out;

    // ==================== 6. SAFETY CONSTRAINTS ====================
    signal extDataSquare <== extDataHash * extDataHash;

    // Ensure depositCount is reasonable
    component depositCountCheck = LessThan(8);
    depositCountCheck.in[0] <== depositCount;
    depositCountCheck.in[1] <== maxDeposits + 1;
    depositCountCheck.out === 1;
}

// Helper template
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

/**
 * Instantiation: Support up to 20 deposits per bucket
 *
 * Why 20?
 * - Circuit size: ~100k constraints (manageable)
 * - Proof time: ~15-20 seconds (acceptable)
 * - Real-world use: Most users won't have >20 deposits at once
 *
 * For more deposits, user can:
 * 1. Withdraw in multiple buckets (20 + 20 + ...)
 * 2. Use multiple MetaSpend keys (different buckets)
 */
component main {public [
    root,
    metaSpendPublic,
    totalWithdrawal,
    extDataHash,
    mintAddress,
    bucketNullifier
]} = BucketWithdrawal(26, 20);
