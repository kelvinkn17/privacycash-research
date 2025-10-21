pragma circom 2.0.0;

include "../../../node_modules/circomlib/circuits/poseidon.circom";

/**
 * PIVY Keypair Circuit
 * Derives public key from private key using Poseidon hash
 *
 * publicKey = Poseidon(privateKey)
 */
template Keypair() {
    signal input privateKey;
    signal output publicKey;

    component hasher = Poseidon(1);
    hasher.inputs[0] <== privateKey;
    publicKey <== hasher.out;
}

/**
 * Signature Circuit
 * Creates a signature used in nullifier computation
 *
 * signature = Poseidon(privateKey, commitment, merklePath)
 */
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
