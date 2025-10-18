# PIVY Implementation Guide: From Concept to Code

**Date**: October 19, 2025
**Version**: 1.0
**Target**: Development team building PIVY protocol

---

## Table of Contents

1. [Development Setup](#development-setup)
2. [Phase 1 MVP: Micro Payment Pool](#phase-1-mvp-micro-payment-pool)
3. [ZK Circuit Development](#zk-circuit-development)
4. [Smart Contract Development](#smart-contract-development)
5. [Frontend & SDK](#frontend--sdk)
6. [Testing Strategy](#testing-strategy)
7. [Deployment](#deployment)
8. [Security Considerations](#security-considerations)

---

## Development Setup

### Prerequisites

```bash
# Solana toolchain
sh -c "$(curl -sSfL https://release.solana.com/v1.18.0/install)"

# Anchor framework
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
avm install latest
avm use latest

# Node.js & TypeScript
nvm install 18
nvm use 18
npm install -g typescript ts-node

# Circom 2
git clone https://github.com/iden3/circom.git
cd circom
cargo build --release
cargo install --path circom

# Snarkjs
npm install -g snarkjs

# Development tools
cargo install solana-verify
cargo install cargo-expand
```

### Project Structure

```
pivy/
├── anchor/
│   ├── programs/
│   │   └── pivy/
│   │       ├── src/
│   │       │   ├── lib.rs                 # Main program
│   │       │   ├── state.rs               # Account structures
│   │       │   ├── instructions/
│   │       │   │   ├── initialize.rs
│   │       │   │   ├── deposit.rs
│   │       │   │   ├── withdraw.rs
│   │       │   │   └── update_sanctioned_list.rs
│   │       │   ├── compliance/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── proof_verifier.rs
│   │       │   │   └── sanctioned_list.rs
│   │       │   ├── merkle_tree.rs
│   │       │   ├── groth16.rs
│   │       │   ├── utils.rs
│   │       │   └── errors.rs
│   │       └── Cargo.toml
│   ├── tests/
│   │   ├── pivy.ts
│   │   └── lib/
│   ├── Anchor.toml
│   └── package.json
│
├── circuits/
│   ├── base/
│   │   ├── transaction.circom           # Core privacy (from Privacy Cash)
│   │   ├── merkleProof.circom
│   │   └── keypair.circom
│   ├── compliance/
│   │   ├── compliance_proof.circom      # NEW: Non-membership proofs
│   │   ├── timing_proof.circom          # NEW: Hold time proofs
│   │   └── structuring_proof.circom     # NEW: Pattern proofs
│   ├── pivy_micro.circom                # Micro tier circuit
│   ├── pivy_standard.circom             # Standard tier circuit
│   ├── pivy_large.circom                # Large tier circuit
│   └── build.sh
│
├── sdk/
│   ├── src/
│   │   ├── index.ts
│   │   ├── PIVYClient.ts
│   │   ├── ProofGenerator.ts
│   │   ├── ComplianceProver.ts
│   │   ├── types.ts
│   │   └── utils.ts
│   ├── package.json
│   └── tsconfig.json
│
├── frontend/
│   ├── src/
│   │   ├── components/
│   │   ├── hooks/
│   │   ├── lib/
│   │   └── app/
│   ├── public/
│   ├── package.json
│   └── next.config.js
│
├── scripts/
│   ├── setup-sanctioned-list.ts
│   ├── deploy.ts
│   └── verify.ts
│
└── docs/
    ├── architecture.md
    ├── api-reference.md
    └── integration-guide.md
```

---

## Phase 1 MVP: Micro Payment Pool

### Goal

Build a working protocol with:
- Single Micro Payment pool ($0-500 limit)
- Basic compliance proofs (non-sanctioned addresses)
- Public compliance dashboard
- Frontend for deposits/withdrawals
- SDK for integrations

### Step 1: Adapt Privacy Cash's Core Privacy Circuit

**Start with `transaction.circom` from Privacy Cash**:

```bash
# Clone Privacy Cash as reference
git clone https://github.com/Privacy-Cash/privacy-cash.git privacy-cash-ref

# Copy base circuits
cp -r privacy-cash-ref/circuits/transaction.circom circuits/base/
cp -r privacy-cash-ref/circuits/merkleProof.circom circuits/base/
cp -r privacy-cash-ref/circuits/keypair.circom circuits/base/
```

**Verify base circuit compiles**:

```bash
cd circuits
circom base/transaction.circom --r1cs --wasm --sym -o build/
```

### Step 2: Create Compliance Proof Circuit

**File**: `circuits/compliance/compliance_proof.circom`

```circom
pragma circom 2.0.0;

include "../../node_modules/circomlib/circuits/poseidon.circom";
include "../../node_modules/circomlib/circuits/comparators.circom";

// Simplified version for MVP (supports up to 100 sanctioned addresses)
template ComplianceProof(maxSanctionedAddresses) {
    // Public inputs
    signal input sanctionedListRoot;

    // Private inputs
    signal input sourceAddress;
    signal input sanctionedAddresses[maxSanctionedAddresses];

    // Prove sourceAddress is NOT in sanctionedAddresses
    component isNotSanctioned[maxSanctionedAddresses];
    signal cleanCheck;

    var allClean = 1;
    for (var i = 0; i < maxSanctionedAddresses; i++) {
        isNotSanctioned[i] = IsEqual();
        isNotSanctioned[i].in[0] <== sourceAddress;
        isNotSanctioned[i].in[1] <== sanctionedAddresses[i];

        // If any match found, allClean becomes 0
        allClean = allClean * (1 - isNotSanctioned[i].out);
    }

    cleanCheck <== allClean;
    cleanCheck === 1;  // Circuit fails if sanctioned

    // Verify sanctioned list hash
    component listHasher = Poseidon(maxSanctionedAddresses);
    for (var i = 0; i < maxSanctionedAddresses; i++) {
        listHasher.inputs[i] <== sanctionedAddresses[i];
    }
    listHasher.out === sanctionedListRoot;

    // Output compliance attestation
    signal output isCompliant;
    isCompliant <== cleanCheck;
}

// For MVP: Support up to 100 sanctioned addresses
component main {public [sanctionedListRoot]} = ComplianceProof(100);
```

**Compile and generate keys**:

```bash
# Compile circuit
circom compliance/compliance_proof.circom --r1cs --wasm --sym -o build/compliance/

# Generate proving/verification keys (use powers of tau ceremony)
cd build/compliance
snarkjs powersoftau new bn128 14 pot14_0000.ptau -v
snarkjs powersoftau contribute pot14_0000.ptau pot14_0001.ptau --name="First contribution" -v
snarkjs powersoftau prepare phase2 pot14_0001.ptau pot14_final.ptau -v

# Generate zkey
snarkjs groth16 setup compliance_proof.r1cs pot14_final.ptau compliance_proof_0000.zkey
snarkjs zkey contribute compliance_proof_0000.zkey compliance_proof_0001.zkey --name="Second contribution" -v
snarkjs zkey export verificationkey compliance_proof_0001.zkey verification_key.json

# Export Solana-compatible verification key
snarkjs zkey export solidityverifier compliance_proof_0001.zkey verifier.sol
# Convert to Rust format (custom script)
node ../../../scripts/convert-vkey-to-rust.js verification_key.json > ../../../anchor/programs/pivy/src/compliance/compliance_vkey.rs
```

### Step 3: Integrate Compliance into Main Circuit

**File**: `circuits/pivy_micro.circom`

```circom
pragma circom 2.0.0;

include "./base/transaction.circom";
include "./compliance/compliance_proof.circom";

// PIVY Micro Tier Transaction Circuit
template PIVYMicroTransaction(levels, nIns, nOuts, maxSanctionedAddresses) {
    // Base transaction signals
    signal input root;
    signal input publicAmount;
    signal input extDataHash;
    signal input mintAddress;
    signal input inputNullifier[nIns];
    signal input inAmount[nIns];
    signal input inPrivateKey[nIns];
    signal input inBlinding[nIns];
    signal input inPathIndices[nIns];
    signal input inPathElements[nIns][levels];
    signal input outputCommitment[nOuts];
    signal input outAmount[nOuts];
    signal input outPubkey[nOuts];
    signal input outBlinding[nOuts];

    // Compliance signals (NEW)
    signal input sanctionedListRoot;
    signal input sourceAddress[nIns];
    signal input sanctionedAddresses[maxSanctionedAddresses];

    // Execute base privacy transaction
    component baseTransaction = Transaction(levels, nIns, nOuts);
    baseTransaction.root <== root;
    baseTransaction.publicAmount <== publicAmount;
    baseTransaction.extDataHash <== extDataHash;
    baseTransaction.mintAddress <== mintAddress;

    for (var i = 0; i < nIns; i++) {
        baseTransaction.inputNullifier[i] <== inputNullifier[i];
        baseTransaction.inAmount[i] <== inAmount[i];
        baseTransaction.inPrivateKey[i] <== inPrivateKey[i];
        baseTransaction.inBlinding[i] <== inBlinding[i];
        baseTransaction.inPathIndices[i] <== inPathIndices[i];
        for (var j = 0; j < levels; j++) {
            baseTransaction.inPathElements[i][j] <== inPathElements[i][j];
        }
    }

    for (var i = 0; i < nOuts; i++) {
        baseTransaction.outputCommitment[i] <== outputCommitment[i];
        baseTransaction.outAmount[i] <== outAmount[i];
        baseTransaction.outPubkey[i] <== outPubkey[i];
        baseTransaction.outBlinding[i] <== outBlinding[i];
    }

    // Execute compliance proofs for each input (NEW)
    component complianceCheck[nIns];
    for (var i = 0; i < nIns; i++) {
        complianceCheck[i] = ComplianceProof(maxSanctionedAddresses);
        complianceCheck[i].sanctionedListRoot <== sanctionedListRoot;
        complianceCheck[i].sourceAddress <== sourceAddress[i];
        for (var j = 0; j < maxSanctionedAddresses; j++) {
            complianceCheck[i].sanctionedAddresses[j] <== sanctionedAddresses[j];
        }
    }

    // Output: Valid transaction that satisfies BOTH privacy AND compliance
    signal output isValid;
    isValid <== 1;
}

// PIVY Micro tier: height 26, 2 inputs, 2 outputs, 100 max sanctioned addresses
component main {public [root, publicAmount, extDataHash, sanctionedListRoot]} = PIVYMicroTransaction(26, 2, 2, 100);
```

**Build script** (`circuits/build.sh`):

```bash
#!/bin/bash

echo "Building PIVY Circuits..."

# Micro tier circuit
echo "Compiling Micro tier circuit..."
circom pivy_micro.circom --r1cs --wasm --sym -o build/micro/

echo "Generating Micro tier keys..."
cd build/micro
snarkjs groth16 setup pivy_micro.r1cs ../../powersoftau/pot14_final.ptau pivy_micro_0000.zkey
snarkjs zkey contribute pivy_micro_0000.zkey pivy_micro_final.zkey --name="Micro tier final" -v
snarkjs zkey export verificationkey pivy_micro_final.zkey verification_key.json

echo "Exporting Rust verification key..."
node ../../scripts/convert-vkey-to-rust.js verification_key.json > ../../anchor/programs/pivy/src/compliance/micro_vkey.rs

echo "✓ Micro tier circuit built successfully"
```

### Step 4: Smart Contract - State Structures

**File**: `anchor/programs/pivy/src/state.rs`

```rust
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PIVYPoolTier {
    MicroPayments,
    StandardPayments,
    LargePayments,
}

#[account(zero_copy)]
pub struct PIVYPoolAccount {
    pub tier: PIVYPoolTier,
    pub authority: Pubkey,

    // Merkle tree state
    pub merkle_root: [u8; 32],
    pub next_index: u64,
    pub subtrees: [[u8; 32]; 26],
    pub root_history: [[u8; 32]; 100],
    pub root_index: u64,

    // Tier-specific limits
    pub max_deposit: u64,
    pub max_withdrawal_daily: u64,
    pub min_hold_time_seconds: i64,

    // Fee structure
    pub deposit_fee_rate: u16,
    pub withdrawal_fee_rate: u16,

    // Statistics
    pub total_deposits: u64,
    pub total_withdrawals: u64,
    pub unique_commitments: u64,

    pub height: u8,
    pub root_history_size: u8,
    pub bump: u8,
    pub _padding: [u8; 5],
}

#[account]
pub struct SanctionedListAccount {
    pub merkle_root: [u8; 32],
    pub version: u64,
    pub last_updated: i64,
    pub authority: Pubkey,
    pub addresses_count: u32,
    pub bump: u8,
}

#[account]
pub struct NullifierAccount {
    pub bump: u8,
}

#[account]
pub struct PIVYComplianceDashboard {
    // Pool statistics
    pub total_deposits_micro: u64,
    pub total_withdrawals_micro: u64,
    pub anonymity_set_size_micro: u64,

    // Compliance metrics
    pub compliance_proofs_verified: u64,
    pub rejected_sanctioned_addresses: u64,

    // Timing metrics
    pub average_hold_time_seconds: u64,

    // Updated timestamp
    pub last_updated: i64,
    pub bump: u8,
}

#[account]
pub struct PIVYGlobalConfig {
    pub authority: Pubkey,
    pub sanctioned_list: Pubkey,
    pub compliance_dashboard: Pubkey,
    pub fee_recipient: Pubkey,
    pub emergency_paused: bool,
    pub bump: u8,
}

// Proof structures
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Proof {
    pub proof_a: [u8; 64],
    pub proof_b: [u8; 128],
    pub proof_c: [u8; 64],
    pub root: [u8; 32],
    pub public_amount: [u8; 32],
    pub ext_data_hash: [u8; 32],
    pub input_nullifiers: [[u8; 32]; 2],
    pub output_commitments: [[u8; 32]; 2],
    pub sanctioned_list_root: [u8; 32],  // NEW
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExtDataMinified {
    pub ext_amount: i64,
    pub fee: u64,
}
```

### Step 5: Smart Contract - Core Logic

**File**: `anchor/programs/pivy/src/lib.rs`

```rust
use anchor_lang::prelude::*;
use light_hasher::Poseidon;

declare_id!("PIVY11111111111111111111111111111111111111111");

pub mod state;
pub mod merkle_tree;
pub mod compliance;
pub mod groth16;
pub mod utils;
pub mod errors;

use state::*;
use merkle_tree::MerkleTree;
use compliance::verify_compliance_proof;

const MERKLE_TREE_HEIGHT: u8 = 26;

#[program]
pub mod pivy {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        // Initialize Micro Payment pool
        let pool = &mut ctx.accounts.pool_account.load_init()?;
        pool.tier = PIVYPoolTier::MicroPayments;
        pool.authority = ctx.accounts.authority.key();
        pool.next_index = 0;
        pool.root_index = 0;
        pool.max_deposit = 500_000_000;           // 0.5 SOL
        pool.max_withdrawal_daily = 2_000_000_000; // 2 SOL
        pool.min_hold_time_seconds = 0;            // No hold time for micro
        pool.deposit_fee_rate = 0;
        pool.withdrawal_fee_rate = 10;             // 0.1%
        pool.height = MERKLE_TREE_HEIGHT;
        pool.root_history_size = 100;
        pool.bump = ctx.bumps.pool_account;

        MerkleTree::initialize::<Poseidon>(pool)?;

        // Initialize sanctioned list
        let sanctioned_list = &mut ctx.accounts.sanctioned_list;
        sanctioned_list.merkle_root = [0u8; 32];  // Empty initially
        sanctioned_list.version = 0;
        sanctioned_list.last_updated = Clock::get()?.unix_timestamp;
        sanctioned_list.authority = ctx.accounts.authority.key();
        sanctioned_list.addresses_count = 0;
        sanctioned_list.bump = ctx.bumps.sanctioned_list;

        // Initialize compliance dashboard
        let dashboard = &mut ctx.accounts.compliance_dashboard;
        dashboard.total_deposits_micro = 0;
        dashboard.total_withdrawals_micro = 0;
        dashboard.anonymity_set_size_micro = 0;
        dashboard.compliance_proofs_verified = 0;
        dashboard.rejected_sanctioned_addresses = 0;
        dashboard.average_hold_time_seconds = 0;
        dashboard.last_updated = Clock::get()?.unix_timestamp;
        dashboard.bump = ctx.bumps.compliance_dashboard;

        // Initialize global config
        let config = &mut ctx.accounts.global_config;
        config.authority = ctx.accounts.authority.key();
        config.sanctioned_list = ctx.accounts.sanctioned_list.key();
        config.compliance_dashboard = ctx.accounts.compliance_dashboard.key();
        config.fee_recipient = ctx.accounts.authority.key();  // Authority initially
        config.emergency_paused = false;
        config.bump = ctx.bumps.global_config;

        msg!("PIVY Protocol initialized successfully");
        Ok(())
    }

    pub fn deposit(
        ctx: Context<Deposit>,
        proof: Proof,
        ext_data_minified: ExtDataMinified,
        encrypted_output1: Vec<u8>,
        encrypted_output2: Vec<u8>,
    ) -> Result<()> {
        let pool = &mut ctx.accounts.pool_account.load_mut()?;
        let config = &ctx.accounts.global_config;
        let sanctioned_list = &ctx.accounts.sanctioned_list;

        // Check not paused
        require!(!config.emergency_paused, ErrorCode::EmergencyPaused);

        // Verify proof root is known
        require!(
            MerkleTree::is_known_root(&pool, proof.root),
            ErrorCode::UnknownRoot
        );

        // Check deposit limit
        let deposit_amount = ext_data_minified.ext_amount as u64;
        require!(
            deposit_amount <= pool.max_deposit,
            ErrorCode::DepositLimitExceeded
        );

        // Verify compliance proof (NEW)
        require!(
            proof.sanctioned_list_root == sanctioned_list.merkle_root,
            ErrorCode::OutdatedComplianceProof
        );

        require!(
            verify_compliance_proof(&proof),
            ErrorCode::InvalidComplianceProof
        );

        // Verify ext data hash
        let ext_data = ExtData::from_minified(&ctx, ext_data_minified);
        let calculated_ext_data_hash = utils::calculate_complete_ext_data_hash(
            ext_data.recipient,
            ext_data.ext_amount,
            &encrypted_output1,
            &encrypted_output2,
            ext_data.fee,
            ext_data.fee_recipient,
            ext_data.mint_address,
        )?;

        require!(
            calculated_ext_data_hash == proof.ext_data_hash,
            ErrorCode::ExtDataHashMismatch
        );

        // Verify ZK proof
        require!(
            utils::verify_proof(proof.clone()),
            ErrorCode::InvalidProof
        );

        // Transfer SOL to pool
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.signer.to_account_info(),
                    to: ctx.accounts.pool_token_account.to_account_info(),
                },
            ),
            deposit_amount,
        )?;

        // Add commitments to merkle tree
        let next_index = pool.next_index;
        MerkleTree::append::<Poseidon>(proof.output_commitments[0], pool)?;
        MerkleTree::append::<Poseidon>(proof.output_commitments[1], pool)?;

        // Emit commitment events
        emit!(CommitmentData {
            index: next_index,
            commitment: proof.output_commitments[0],
            encrypted_output: encrypted_output1,
        });

        emit!(CommitmentData {
            index: next_index + 1,
            commitment: proof.output_commitments[1],
            encrypted_output: encrypted_output2,
        });

        // Update dashboard
        let dashboard = &mut ctx.accounts.compliance_dashboard;
        dashboard.total_deposits_micro += deposit_amount;
        dashboard.anonymity_set_size_micro += 2;
        dashboard.compliance_proofs_verified += 1;
        dashboard.last_updated = Clock::get()?.unix_timestamp;

        msg!("Deposit successful: {} lamports", deposit_amount);
        Ok(())
    }

    pub fn withdraw(
        ctx: Context<Withdraw>,
        proof: Proof,
        ext_data_minified: ExtDataMinified,
        encrypted_output1: Vec<u8>,
        encrypted_output2: Vec<u8>,
    ) -> Result<()> {
        // Similar to deposit but for withdrawal
        // (Implementation mirrors Privacy Cash with compliance verification added)
        // ...

        Ok(())
    }

    pub fn update_sanctioned_list(
        ctx: Context<UpdateSanctionedList>,
        new_root: [u8; 32],
        new_addresses_count: u32,
    ) -> Result<()> {
        let sanctioned_list = &mut ctx.accounts.sanctioned_list;

        sanctioned_list.merkle_root = new_root;
        sanctioned_list.version += 1;
        sanctioned_list.addresses_count = new_addresses_count;
        sanctioned_list.last_updated = Clock::get()?.unix_timestamp;

        emit!(SanctionedListUpdated {
            version: sanctioned_list.version,
            addresses_count: new_addresses_count,
        });

        msg!("Sanctioned list updated to version {}", sanctioned_list.version);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<PIVYPoolAccount>(),
        seeds = [b"pivy_pool", &[PIVYPoolTier::MicroPayments as u8]],
        bump
    )]
    pub pool_account: AccountLoader<'info, PIVYPoolAccount>,

    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<SanctionedListAccount>(),
        seeds = [b"sanctioned_list"],
        bump
    )]
    pub sanctioned_list: Account<'info, SanctionedListAccount>,

    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<PIVYComplianceDashboard>(),
        seeds = [b"compliance_dashboard"],
        bump
    )]
    pub compliance_dashboard: Account<'info, PIVYComplianceDashboard>,

    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<PIVYGlobalConfig>(),
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, PIVYGlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// ... (other context structs similar to Privacy Cash)

#[event]
pub struct CommitmentData {
    pub index: u64,
    pub commitment: [u8; 32],
    pub encrypted_output: Vec<u8>,
}

#[event]
pub struct SanctionedListUpdated {
    pub version: u64,
    pub addresses_count: u32,
}
```

### Step 6: Compliance Proof Verifier

**File**: `anchor/programs/pivy/src/compliance/proof_verifier.rs`

```rust
use crate::state::Proof;
use crate::groth16::{Groth16Verifier, Groth16Verifyingkey};

// Import generated verification key from circuit compilation
mod micro_vkey;
use micro_vkey::COMPLIANCE_VERIFYING_KEY;

pub fn verify_compliance_proof(proof: &Proof) -> bool {
    // Public inputs for compliance proof
    let mut public_inputs: [[u8; 32]; 1] = [[0u8; 32]; 1];
    public_inputs[0] = proof.sanctioned_list_root;

    // Verify groth16 proof
    let mut verifier = match Groth16Verifier::new(
        &proof.proof_a,
        &proof.proof_b,
        &proof.proof_c,
        &public_inputs,
        &COMPLIANCE_VERIFYING_KEY,
    ) {
        Ok(v) => v,
        Err(_) => return false,
    };

    verifier.verify().unwrap_or(false)
}
```

### Step 7: SDK for Proof Generation

**File**: `sdk/src/ComplianceProver.ts`

```typescript
import { PublicKey } from '@solana/web3.js';
import { buildPoseidon } from 'circomlibjs';
const snarkjs = require('snarkjs');

export class ComplianceProver {
  private poseidon: any;
  private wasmPath: string;
  private zkeyPath: string;

  constructor(wasmPath: string, zkeyPath: string) {
    this.wasmPath = wasmPath;
    this.zkeyPath = zkeyPath;
  }

  async initialize() {
    this.poseidon = await buildPoseidon();
  }

  /**
   * Generate compliance proof that sourceAddress is NOT in sanctioned list
   */
  async generateComplianceProof(
    sourceAddress: PublicKey,
    sanctionedList: PublicKey[],
    maxSize: number = 100
  ): Promise<{
    proof: any;
    publicSignals: any;
  }> {
    // Pad sanctioned list to max size
    const paddedList = [...sanctionedList];
    while (paddedList.length < maxSize) {
      paddedList.push(PublicKey.default);
    }

    // Calculate sanctioned list root
    const sanctionedListRoot = this.poseidon.F.toString(
      this.poseidon(paddedList.map(pk => pk.toBuffer()))
    );

    // Circuit inputs
    const input = {
      sanctionedListRoot: sanctionedListRoot,
      sourceAddress: sourceAddress.toBuffer(),
      sanctionedAddresses: paddedList.map(pk => pk.toBuffer()),
    };

    // Generate proof
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(
      input,
      this.wasmPath,
      this.zkeyPath
    );

    return { proof, publicSignals };
  }

  /**
   * Check if address is in sanctioned list (client-side validation)
   */
  isAddressSanctioned(
    address: PublicKey,
    sanctionedList: PublicKey[]
  ): boolean {
    return sanctionedList.some(sanctioned =>
      sanctioned.equals(address)
    );
  }

  /**
   * Fetch current sanctioned list from on-chain registry
   */
  async fetchSanctionedList(
    program: any,
    sanctionedListPDA: PublicKey
  ): Promise<{
    root: Buffer;
    version: number;
    addresses: PublicKey[];
  }> {
    const account = await program.account.sanctionedListAccount.fetch(
      sanctionedListPDA
    );

    // Fetch full list from off-chain storage (IPFS/Arweave)
    // (Address list is too large to store on-chain)
    const response = await fetch(
      `https://pivy-sanctioned-lists.arweave.net/${account.version}.json`
    );
    const addresses = await response.json();

    return {
      root: Buffer.from(account.merkleRoot),
      version: account.version.toNumber(),
      addresses: addresses.map((addr: string) => new PublicKey(addr)),
    };
  }
}
```

### Step 8: Frontend Integration

**File**: `frontend/src/components/DepositForm.tsx`

```typescript
import { useState } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';
import { PIVYClient } from '@pivy/sdk';

export function DepositForm() {
  const wallet = useWallet();
  const [amount, setAmount] = useState('');
  const [loading, setLoading] = useState(false);

  const handleDeposit = async () => {
    if (!wallet.publicKey) return;

    setLoading(true);
    try {
      const client = new PIVYClient(connection, wallet);

      // Generate compliance proof
      const sanctionedList = await client.fetchSanctionedList();

      // Check if user is sanctioned (client-side)
      if (client.isAddressSanctioned(wallet.publicKey, sanctionedList.addresses)) {
        alert('Error: Your address is on the sanctioned list');
        return;
      }

      // Generate full proof (privacy + compliance)
      const proof = await client.generateDepositProof({
        amount: parseFloat(amount),
        sourceAddress: wallet.publicKey,
        sanctionedList: sanctionedList.addresses,
      });

      // Submit transaction
      const signature = await client.deposit(proof);

      alert(`Deposit successful! Signature: ${signature}`);
    } catch (error) {
      console.error('Deposit failed:', error);
      alert(`Deposit failed: ${error.message}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="deposit-form">
      <h2>Deposit to PIVY</h2>
      <input
        type="number"
        value={amount}
        onChange={(e) => setAmount(e.target.value)}
        placeholder="Amount (SOL)"
        max="0.5"
      />
      <button onClick={handleDeposit} disabled={loading || !wallet.publicKey}>
        {loading ? 'Processing...' : 'Deposit'}
      </button>
      <p className="info">Max deposit: 0.5 SOL (Micro tier)</p>
    </div>
  );
}
```

---

## Testing Strategy

### Unit Tests

```typescript
// anchor/tests/pivy.ts

describe('PIVY Protocol', () => {
  it('Initialize protocol', async () => {
    const tx = await program.methods
      .initialize()
      .accounts({
        poolAccount: poolPDA,
        sanctionedList: sanctionedListPDA,
        complianceDashboard: dashboardPDA,
        globalConfig: configPDA,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    const pool = await program.account.pivyPoolAccount.fetch(poolPDA);
    expect(pool.tier).to.equal('MicroPayments');
    expect(pool.maxDeposit.toNumber()).to.equal(500_000_000);
  });

  it('Reject deposit from sanctioned address', async () => {
    // Add test address to sanctioned list
    const testSanctionedAddress = Keypair.generate().publicKey;
    await program.methods
      .updateSanctionedList(
        calculateMerkleRoot([testSanctionedAddress]),
        1
      )
      .accounts({
        sanctionedList: sanctionedListPDA,
        authority: authority.publicKey,
      })
      .signers([authority])
      .rpc();

    // Try to generate proof with sanctioned address
    try {
      await client.generateDepositProof({
        amount: 0.1,
        sourceAddress: testSanctionedAddress,
        sanctionedList: [testSanctionedAddress],
      });
      expect.fail('Should have thrown error');
    } catch (error) {
      expect(error.message).to.include('sanctioned');
    }
  });

  it('Accept deposit from clean address', async () => {
    const cleanUser = Keypair.generate();

    // Airdrop SOL to clean user
    await connection.requestAirdrop(cleanUser.publicKey, 1e9);

    // Generate proof
    const proof = await client.generateDepositProof({
      amount: 0.1,
      sourceAddress: cleanUser.publicKey,
      sanctionedList: [], // Empty list
    });

    // Submit deposit
    const signature = await client.deposit(proof);

    // Verify dashboard updated
    const dashboard = await program.account.pivyComplianceDashboard.fetch(dashboardPDA);
    expect(dashboard.totalDepositsMicro.toNumber()).to.be.greaterThan(0);
    expect(dashboard.complianceProofsVerified.toNumber()).to.equal(1);
  });

  it('Enforce deposit limit', async () => {
    try {
      await client.deposit({
        amount: 1.0, // Exceeds 0.5 SOL limit
        sourceAddress: user.publicKey,
      });
      expect.fail('Should have thrown error');
    } catch (error) {
      expect(error.message).to.include('DepositLimitExceeded');
    }
  });
});
```

### Integration Tests

```bash
# Test full deposit-withdraw cycle
anchor test -- --features localnet

# Test with real sanctioned list
SANCTIONED_LIST_URL=https://ofac.treasury.gov/sanctioned-addresses anchor test

# Load testing
k6 run tests/load-test.js
```

---

## Deployment

### Testnet Deployment

```bash
# Build program
anchor build --verifiable

# Deploy to devnet
anchor deploy --provider.cluster devnet

# Initialize protocol
ts-node scripts/initialize-protocol.ts --cluster devnet

# Upload sanctioned list
ts-node scripts/setup-sanctioned-list.ts --cluster devnet
```

### Mainnet Deployment

```bash
# Build verifiable program
anchor build --verifiable

# Verify build
solana-verify build

# Deploy to mainnet
anchor deploy --provider.cluster mainnet

# Transfer authority to multisig
solana program set-upgrade-authority PROGRAM_ID \
  --new-upgrade-authority MULTISIG_ADDRESS

# Initialize with production config
ts-node scripts/initialize-protocol.ts --cluster mainnet --production
```

---

## Security Considerations

### Audit Checklist

1. **ZK Circuit Security**
   - [ ] Soundness: Circuit cannot be bypassed
   - [ ] Completeness: Valid proofs always verify
   - [ ] Zero-knowledge: No information leaked
   - [ ] Constraint coverage: All paths verified
   - [ ] Trusted setup: Proper powers of tau ceremony

2. **Smart Contract Security**
   - [ ] Reentrancy protection
   - [ ] Integer overflow/underflow
   - [ ] Access control (authority checks)
   - [ ] Account validation
   - [ ] PDA derivation correctness
   - [ ] Nullifier uniqueness

3. **Compliance Security**
   - [ ] Sanctioned list updates (only authority)
   - [ ] Proof expiration handling
   - [ ] List version synchronization
   - [ ] Emergency pause functionality

### Bug Bounty

Launch bug bounty program on Immunefi:
- Critical: $50K-$100K
- High: $10K-$50K
- Medium: $5K-$10K
- Low: $1K-$5K

---

**End of Implementation Guide**

Next steps:
1. Set up development environment
2. Port Privacy Cash circuits
3. Build compliance proofs
4. Develop smart contracts
5. Create SDK
6. Build frontend
7. Test extensively
8. Audit
9. Deploy

Let's build the future of compliant privacy! 🚀
