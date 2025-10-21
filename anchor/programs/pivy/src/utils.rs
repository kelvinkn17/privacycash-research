use crate::WithdrawalProof;
use crate::groth16::{Groth16Verifier, Groth16Verifyingkey};
use anchor_lang::prelude::*;
use ark_bn254;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Compress, Validate};
use std::ops::Neg;

type G1 = ark_bn254::g1::G1Affine;

// Placeholder verifying key - to be replaced with actual PIVY circuit verifying key
pub const VERIFYING_KEY: Groth16Verifyingkey = Groth16Verifyingkey {
    nr_pubinputs: 3, // bucket_root, nullifier, meta_spend_pubkey

    vk_alpha_g1: [
        45,77,154,167,227,2,217,223,65,116,157,85,7,148,157,5,219,234,51,251,177,108,100,59,34,245,153,162,190,109,242,226,
        20,190,221,80,60,55,206,176,97,216,236,96,32,159,227,69,206,137,131,10,25,35,3,1,240,118,202,255,0,77,25,38,
    ],

    vk_beta_g2: [
        9,103,3,47,203,247,118,209,175,201,133,248,136,119,241,130,211,132,128,166,83,242,222,202,169,121,76,188,59,243,6,12,
        14,24,120,71,173,76,121,131,116,208,214,115,43,245,1,132,125,214,139,192,224,113,36,30,2,19,188,127,193,61,183,171,
        48,76,251,209,224,138,112,74,153,245,232,71,217,63,140,60,170,253,222,196,107,122,13,55,157,166,154,77,17,35,70,167,
        23,57,193,177,164,87,168,199,49,49,35,210,77,47,145,146,248,150,183,198,62,234,5,169,213,127,6,84,122,208,206,200,
    ],

    vk_gamme_g2: [
        25,142,147,147,146,13,72,58,114,96,191,183,49,251,93,37,241,170,73,51,53,169,231,18,151,228,133,183,174,243,18,194,
        24,0,222,239,18,31,30,118,66,106,0,102,94,92,68,121,103,67,34,212,247,94,218,221,70,222,189,92,217,146,246,237,
        9,6,137,208,88,95,240,117,236,158,153,173,105,12,51,149,188,75,49,51,112,179,142,243,85,172,218,220,209,34,151,91,
        18,200,94,165,219,140,109,235,74,171,113,128,141,203,64,143,227,209,231,105,12,67,211,123,76,230,204,1,102,250,125,170,
    ],

    vk_delta_g2: [
        25,252,204,73,0,218,132,40,192,175,106,179,247,34,6,163,111,68,46,211,76,146,16,158,28,23,146,254,157,94,7,92,
        34,128,9,143,49,11,128,172,203,141,109,166,180,82,110,179,223,71,56,138,77,154,73,160,146,198,203,125,196,135,167,56,
        21,152,106,224,184,3,47,85,250,118,220,185,175,242,111,30,40,24,69,173,252,13,109,1,241,162,122,76,24,38,72,88,
        45,118,91,197,236,236,152,29,29,233,108,250,155,255,230,156,182,159,1,3,41,60,40,136,181,220,23,150,130,211,23,83,
    ],

    vk_ic: &[
        [
            35,121,23,162,32,101,247,115,177,199,50,158,3,60,188,95,91,29,121,210,53,155,245,226,203,245,186,167,39,32,160,202,
            22,22,168,160,125,45,56,45,132,214,20,198,76,81,2,150,0,61,86,130,105,170,141,244,13,180,81,79,18,166,129,129,
        ],
        [
            13,148,63,234,185,42,3,159,127,24,240,200,72,24,176,7,181,215,212,52,13,160,172,182,177,22,235,4,173,229,25,108,
            46,61,233,184,181,152,132,103,252,100,229,144,217,36,39,254,67,237,70,214,192,231,140,86,113,40,11,88,12,150,157,226,
        ],
        [
            26,105,150,204,178,202,26,62,39,178,179,225,133,140,138,40,60,187,99,57,237,7,203,159,251,103,46,207,219,186,19,64,
            0,42,73,5,76,48,115,80,96,29,197,213,228,240,7,144,140,3,127,89,87,247,98,153,174,81,7,158,183,80,139,147,
        ],
        [
            6,249,88,104,56,74,144,136,129,176,70,216,18,147,78,141,24,93,95,242,68,49,215,152,246,110,151,241,228,59,230,187,
            29,56,186,210,200,190,93,64,110,0,55,105,166,104,208,46,82,81,146,136,179,99,104,232,99,248,162,137,21,217,220,77,
        ],
    ]
};

/// Verify withdrawal proof
/// For PIVY, the proof verifies:
/// - Knowledge of meta_spend private key
/// - Bucket commitments are in the merkle tree
/// - Total amount matches bucket balance
pub fn verify_withdrawal_proof(proof: WithdrawalProof, verifying_key: Groth16Verifyingkey) -> bool {
    let mut public_inputs_vec: [[u8; 32]; 3] = [[0u8; 32]; 3];

    public_inputs_vec[0] = proof.bucket_root;
    public_inputs_vec[1] = proof.nullifier;
    public_inputs_vec[2] = proof.meta_spend_pubkey;

    // First deserialize PROOF_A into a G1 point
    let g1_point = match G1::deserialize_with_mode(
        &*[&change_endianness(&proof.proof_a[0..64]), &[0u8][..]].concat(),
        Compress::No,
        Validate::Yes,
    ) {
        Ok(point) => point,
        Err(_) => return false,
    };

    let mut proof_a_neg = [0u8; 65];
    if g1_point
        .neg()
        .x
        .serialize_with_mode(&mut proof_a_neg[..32], Compress::No)
        .is_err() {
        return false;
    }
    if g1_point
        .neg()
        .y
        .serialize_with_mode(&mut proof_a_neg[32..], Compress::No)
        .is_err() {
        return false;
    }

    let proof_a: [u8; 64] = match change_endianness(&proof_a_neg[..64]).try_into() {
        Ok(array) => array,
        Err(_) => return false,
    };

    let mut verifier = match Groth16Verifier::new(
        &proof_a,
        &proof.proof_b,
        &proof.proof_c,
        &public_inputs_vec,
        &verifying_key
    ) {
        Ok(v) => v,
        Err(_) => return false,
    };

    verifier.verify().unwrap_or(false)
}

pub fn change_endianness(bytes: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    for b in bytes.chunks(32) {
        for byte in b.iter().rev() {
            vec.push(*byte);
        }
    }
    vec
}
