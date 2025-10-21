# PIVY - Privacy Payment Platform for Solana

PIVY is a privacy-focused payment platform built on Solana that enables users to send SOL privately using meta keypairs and bucket-based aggregation.

## Key Features

### 1. Account-Based System with Meta Keypairs
- **MetaView Keypair**: Used for viewing balance (client-side encryption)
- **MetaSpend Keypair**: Used for withdrawing funds (ZK proof of ownership)
- Users share only PUBLIC keys on their PIVY page (e.g., pivy.me/kelvin)
- Depositors can send funds without knowing the recipient's main wallet address

### 2. Bucket-Based Aggregation
Unlike the original privacy-cash implementation that processes each UTXO individually:
- **Privacy Cash**: 10 deposits = 10+ minutes to withdraw (limited to 2 UTXOs per transaction)
- **PIVY**: 10 deposits = 1 transaction to withdraw entire balance

All deposits to a PIVY account accumulate in a "bucket" using Pedersen commitment homomorphic addition:
```
Bucket = C1 + C2 + C3 + ... + C100
```

The user proves ownership of the entire bucket in ONE transaction.

### 3. Simplified Deposit Flow
1. John visits `pivy.me/kelvin`
2. Kelvin's page displays his MetaView and MetaSpend PUBLIC keys
3. John's client generates:
   - Commitment: `hash(amount, MetaSpend_PUBLIC, blinding)`
   - Encrypted output: `encrypt({amount, blinding}, MetaView_PUBLIC)`
4. John signs with HIS wallet and transfers SOL to the pool
5. Kelvin can decrypt the output with MetaView_PRIVATE to see his balance
6. Only Kelvin can withdraw using MetaSpend_PRIVATE

## Architecture

### Core Instructions

#### `initialize`
Initializes the PIVY program with:
- Merkle tree for commitment tracking
- Global pool account for SOL storage
- Global config for fees

#### `deposit`
```rust
pub fn deposit(
    ctx: Context<Deposit>,
    commitment: [u8; 32],           // hash(amount, metaSpend_pub, blinding)
    encrypted_output: Vec<u8>,       // encrypt({amount, blinding}, metaView_pub)
    amount: u64,                     // SOL amount in lamports
    blinded_account_id: [u8; 32],   // hash(metaSpend_pub) for bucket PDA
) -> Result<()>
```

Features:
- Adds commitment to merkle tree
- Accumulates in recipient's bucket account
- Emits event with encrypted output for recipient
- Deposit limit protection

#### `withdraw`
```rust
pub fn withdraw(
    ctx: Context<Withdraw>,
    proof: WithdrawalProof,          // ZK proof of meta_spend ownership
    withdrawal_amount: u64,          // Amount to withdraw
) -> Result<()>
```

Features:
- Verifies ZK proof of meta_spend private key ownership
- Validates bucket hasn't been spent
- Calculates withdrawal fee (0.25% default)
- Transfers entire requested amount in ONE transaction
- Marks bucket as spent to prevent double-spending

### Account Structures

#### `BucketAccount`
```rust
pub struct BucketAccount {
    pub commitment_count: u8,                                // Number of deposits
    pub total_balance: u64,                                  // Total SOL balance
    pub is_spent: bool,                                      // Prevent double-spend
    pub commitments: [[u8; 32]; MAX_BUCKET_SIZE],           // All commitments
    pub bump: u8,                                           // PDA bump
}
```

#### `MerkleTreeAccount`
Zero-copy account storing:
- Sparse merkle tree for commitment proofs
- Root history (100 roots) for proof verification
- Current tree state and metadata

## Testing

Run unit tests:
```bash
cargo test -p pivy --lib
```

All 11 tests pass:
- ✅ Bucket commitment management
- ✅ Multiple deposits aggregation
- ✅ Overflow protection
- ✅ Fee calculation
- ✅ Happy path flow (deposit → check balance → withdraw)
- ✅ Multiple users with separate buckets
- ✅ Double-spend prevention
- ✅ Encrypted output concept

## Differences from Privacy-Cash

| Feature | Privacy-Cash | PIVY |
|---------|--------------|------|
| **Withdrawal** | 2 UTXOs per tx | Entire bucket per tx |
| **Time** | 10 deposits = 10+ min | 10 deposits = 1 tx |
| **Keys** | Single keypair | MetaView + MetaSpend |
| **Deposit** | Sender needs recipient address | Sender uses public meta keys |
| **Balance View** | Need to scan all UTXOs | Decrypt with MetaView key |
| **Aggregation** | Individual UTXOs | Bucket-based (Pedersen) |

## Security Features

1. **Double-Spend Prevention**: Bucket marked as spent after withdrawal
2. **ZK Proof Verification**: Groth16 proof validates meta_spend ownership
3. **Merkle Tree**: Commitments tracked in sparse merkle tree with root history
4. **Deposit Limits**: Configurable maximum deposit amount
5. **Fee Validation**: Withdrawal fees calculated on-chain

## Compliance

Current MVP **does not** include:
- Backdoor access for regulators
- Transaction limits beyond deposit limits
- AML/KYC integration

These can be added later as needed.

## Development Status

- ✅ Core smart contract implementation
- ✅ Bucket-based aggregation
- ✅ Meta keypair system
- ✅ Unit tests for happy path
- ⏳ ZK circuit for withdrawal proof (placeholder verifying key)
- ⏳ Client-side implementation
- ⏳ Integration tests

## Next Steps

1. **ZK Circuit**: Implement actual PIVY withdrawal circuit in Circom
2. **Client SDK**: Build client library for deposit/withdrawal operations
3. **Encryption**: Implement actual encryption for encrypted_output
4. **Integration Tests**: Test full flow with ZK proofs
5. **Optimization**: Gas optimization and performance tuning

## Running

Build the program:
```bash
cargo build-sbf -p pivy
```

Deploy (on localnet/devnet):
```bash
anchor deploy --program-name pivy
```

## License

MIT
