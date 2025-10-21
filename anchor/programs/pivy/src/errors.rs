use anchor_lang::prelude::*;

#[derive(Debug)]
pub enum Groth16Error {
    InvalidG1Length,
    InvalidG2Length,
    InvalidPublicInputsLength,
    PublicInputGreaterThanFieldSize,
    PreparingInputsG1MulFailed,
    PreparingInputsG1AdditionFailed,
    ProofVerificationFailed,
}

impl std::fmt::Display for Groth16Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Groth16Error::InvalidG1Length => write!(f, "Invalid G1 point length"),
            Groth16Error::InvalidG2Length => write!(f, "Invalid G2 point length"),
            Groth16Error::InvalidPublicInputsLength => write!(f, "Invalid public inputs length"),
            Groth16Error::PublicInputGreaterThanFieldSize => {
                write!(f, "Public input greater than field size")
            }
            Groth16Error::PreparingInputsG1MulFailed => {
                write!(f, "Failed to prepare inputs: G1 multiplication failed")
            }
            Groth16Error::PreparingInputsG1AdditionFailed => {
                write!(f, "Failed to prepare inputs: G1 addition failed")
            }
            Groth16Error::ProofVerificationFailed => write!(f, "Proof verification failed"),
        }
    }
}

impl std::error::Error for Groth16Error {}
