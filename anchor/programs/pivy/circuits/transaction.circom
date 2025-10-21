pragma circom 2.0.0;

include "../../../node_modules/circomlib/circuits/poseidon.circom";
include "./merkleProof.circom";
include "./keypair.circom";

/**
 * PIVY Universal Transaction Circuit
 *
 * This circuit handles BOTH deposits and withdrawals:
 * - Deposits: inputUTXOs are empty, publicAmount > 0
 * - Withdrawals: inputUTXOs are real, publicAmount < 0
 * - Transfers: inputUTXOs are real, publicAmount = 0
 *
 * UTXO structure:
 * {
 *   amount,
 *   metaSpendPubkey,
 *   blinding,
 *   mintAddress
 * }
 *
 * commitment = Poseidon(amount, metaSpendPubkey, blinding, mintAddress)
 * nullifier = Poseidon(commitment, merklePath, signature)
 * signature = Poseidon(metaSpendPrivateKey, commitment, merklePath)
 */
template Transaction(levels, nIns, nOuts) {
    // Public inputs (visible on-chain)
    signal input root;
    signal input publicAmount;        // Can be positive (deposit), negative (withdraw), or zero (transfer)
    signal input extDataHash;         // Hash of external data (recipient, fee, etc)
    signal input mintAddress;         // Token mint (SOL or SPL token)

    // Public outputs
    signal input inputNullifier[nIns];
    signal input outputCommitment[nOuts];

    // Private inputs for spending UTXOs
    signal input inAmount[nIns];
    signal input inPrivateKey[nIns];       // MetaSpend private key
    signal input inBlinding[nIns];
    signal input inPathIndices[nIns];
    signal input inPathElements[nIns][levels];

    // Private inputs for creating UTXOs
    signal input outAmount[nOuts];
    signal input outPubkey[nOuts];        // MetaSpend public key
    signal input outBlinding[nOuts];

    // Components for inputs
    component inKeypair[nIns];
    component inSignature[nIns];
    component inCommitmentHasher[nIns];
    component inNullifierHasher[nIns];
    component inTree[nIns];
    component inCheckRoot[nIns];
    var sumIns = 0;

    // Verify correctness of input UTXOs (being spent)
    for (var tx = 0; tx < nIns; tx++) {
        // Derive public key from private key
        inKeypair[tx] = Keypair();
        inKeypair[tx].privateKey <== inPrivateKey[tx];

        // Compute commitment = Poseidon(amount, pubkey, blinding, mint)
        inCommitmentHasher[tx] = Poseidon(4);
        inCommitmentHasher[tx].inputs[0] <== inAmount[tx];
        inCommitmentHasher[tx].inputs[1] <== inKeypair[tx].publicKey;
        inCommitmentHasher[tx].inputs[2] <== inBlinding[tx];
        inCommitmentHasher[tx].inputs[3] <== mintAddress;

        // Create signature for nullifier
        inSignature[tx] = Signature();
        inSignature[tx].privateKey <== inPrivateKey[tx];
        inSignature[tx].commitment <== inCommitmentHasher[tx].out;
        inSignature[tx].merklePath <== inPathIndices[tx];

        // Compute nullifier = Poseidon(commitment, path, signature)
        inNullifierHasher[tx] = Poseidon(3);
        inNullifierHasher[tx].inputs[0] <== inCommitmentHasher[tx].out;
        inNullifierHasher[tx].inputs[1] <== inPathIndices[tx];
        inNullifierHasher[tx].inputs[2] <== inSignature[tx].out;
        inNullifierHasher[tx].out === inputNullifier[tx];

        // Verify Merkle proof (commitment is in tree)
        inTree[tx] = MerkleProof(levels);
        inTree[tx].leaf <== inCommitmentHasher[tx].out;
        inTree[tx].pathIndices <== inPathIndices[tx];
        for (var i = 0; i < levels; i++) {
            inTree[tx].pathElements[i] <== inPathElements[tx][i];
        }

        // Check merkle proof only if amount is non-zero
        inCheckRoot[tx] = ForceEqualIfEnabled();
        inCheckRoot[tx].in[0] <== root;
        inCheckRoot[tx].in[1] <== inTree[tx].root;
        inCheckRoot[tx].enabled <== inAmount[tx];

        sumIns += inAmount[tx];
    }

    // Components for outputs
    component outCommitmentHasher[nOuts];
    component outAmountCheck[nOuts];
    var sumOuts = 0;

    // Verify correctness of output UTXOs (being created)
    for (var tx = 0; tx < nOuts; tx++) {
        // Compute commitment = Poseidon(amount, pubkey, blinding, mint)
        outCommitmentHasher[tx] = Poseidon(4);
        outCommitmentHasher[tx].inputs[0] <== outAmount[tx];
        outCommitmentHasher[tx].inputs[1] <== outPubkey[tx];
        outCommitmentHasher[tx].inputs[2] <== outBlinding[tx];
        outCommitmentHasher[tx].inputs[3] <== mintAddress;
        outCommitmentHasher[tx].out === outputCommitment[tx];

        // Check that amount fits into 248 bits to prevent overflow
        outAmountCheck[tx] = Num2Bits(248);
        outAmountCheck[tx].in <== outAmount[tx];

        sumOuts += outAmount[tx];
    }

    // Check for duplicate nullifiers
    component sameNullifiers[nIns * (nIns - 1) / 2];
    var index = 0;
    for (var i = 0; i < nIns - 1; i++) {
        for (var j = i + 1; j < nIns; j++) {
            sameNullifiers[index] = IsEqual();
            sameNullifiers[index].in[0] <== inputNullifier[i];
            sameNullifiers[index].in[1] <== inputNullifier[j];
            sameNullifiers[index].out === 0;
            index++;
        }
    }

    // Verify amount invariant: inputs + publicAmount == outputs
    // For deposits: 0 + 0 + publicAmount(+5) == 5 + 0
    // For withdrawals: 5 + 0 + publicAmount(-5) == 0 + 0
    sumIns + publicAmount === sumOuts;

    // Safety constraint for extDataHash
    signal extDataSquare <== extDataHash * extDataHash;
}

// Helper template from circomlib
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
