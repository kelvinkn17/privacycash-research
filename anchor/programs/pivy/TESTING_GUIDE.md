# PIVY Testing Guide

## Quick Start

### Run All Tests
```bash
cargo test -p pivy --lib
```

Expected output:
```
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored
```

### Run Specific Test
```bash
cargo test -p pivy --lib test_happy_path_flow
```

### Run Tests with Output
```bash
cargo test -p pivy --lib -- --nocapture
```

---

## Test Coverage

### 1. Basic Bucket Operations
- `test_bucket_account_add_commitment`: Single deposit to bucket
- `test_bucket_account_multiple_deposits`: Multiple deposits aggregate correctly
- `test_bucket_account_overflow_protection`: Prevents balance overflow

### 2. Commitment Generation
- `test_commitment_generation`: Deterministic commitment creation

### 3. Aggregation Concept
- `test_bucket_aggregation_concept`: Demonstrates 10 deposits → 1 withdrawal

### 4. Happy Path Flow
- `test_happy_path_flow`: Complete user journey:
  ```
  John deposits 1000 → Kelvin sees 1000
  Sarah deposits 2000 → Kelvin sees 3000
  Kelvin withdraws 3000 → Success
  ```

### 5. Multi-User Support
- `test_multiple_users_separate_buckets`: Kelvin and Sarah have separate balances

### 6. Security
- `test_withdrawal_prevents_double_spend`: Bucket can't be spent twice

### 7. Fee Calculation
- `test_fee_calculation`: 0.25% withdrawal fee computed correctly

### 8. Encryption Concept
- `test_encrypted_output_concept`: Mock encryption for MetaView

---

## Building the Program

### Development Build
```bash
cargo build -p pivy
```

### Solana BPF Build
```bash
cargo build-sbf -p pivy
```

### Clean Build
```bash
cargo clean -p pivy
cargo build-sbf -p pivy
```

---

## Test Scenarios

### Scenario 1: Single User, Single Deposit
```rust
let mut bucket = BucketAccount::default();
bucket.add_commitment([1u8; 32], 1000).unwrap();

assert_eq!(bucket.total_balance, 1000);
assert_eq!(bucket.commitment_count, 1);
```

### Scenario 2: Single User, Multiple Deposits
```rust
let mut bucket = BucketAccount::default();

// John sends 1000
bucket.add_commitment([1u8; 32], 1000).unwrap();

// Sarah sends 2000
bucket.add_commitment([2u8; 32], 2000).unwrap();

// Mike sends 500
bucket.add_commitment([3u8; 32], 500).unwrap();

assert_eq!(bucket.total_balance, 3500);
assert_eq!(bucket.commitment_count, 3);
```

### Scenario 3: Withdrawal
```rust
// After deposits
assert!(!bucket.is_spent);
assert_eq!(bucket.total_balance, 3500);

// Withdraw
bucket.is_spent = true;

// Can't withdraw again
assert!(bucket.is_spent); // Would cause ErrorCode::BucketAlreadySpent
```

---

## Manual Testing on Devnet

### 1. Deploy Program
```bash
# Build
cargo build-sbf -p pivy

# Deploy
anchor deploy --program-name pivy --provider.cluster devnet
```

### 2. Initialize Program
```bash
anchor test --skip-build --skip-deploy --provider.cluster devnet
```

### 3. Test Deposit
```javascript
// In tests/pivy.ts
await program.methods.deposit(
  commitment,
  encryptedOutput,
  new BN(1000),
  blindedAccountId
).accounts({
  treeAccount,
  bucketAccount,
  poolAccount,
  depositor: depositor.publicKey,
  systemProgram: SystemProgram.programId,
}).signers([depositor]).rpc();
```

### 4. Check Balance
```javascript
// Query deposit events
const events = await program.account.depositEvent.all();

// Decrypt (mock)
for (const event of events) {
  console.log("Commitment:", event.commitment);
  console.log("Encrypted output:", event.encryptedOutput);
}
```

### 5. Test Withdrawal
```javascript
// Generate mock proof
const proof = {
  proofA: new Array(64).fill(0),
  proofB: new Array(128).fill(0),
  proofC: new Array(64).fill(0),
  bucketRoot: bucketCommitmentRoot,
  nullifier: bucketId,
  metaSpendPubkey: metaSpendPublic,
};

await program.methods.withdraw(
  proof,
  new BN(3000)
).accounts({
  treeAccount,
  bucketAccount,
  poolAccount,
  globalConfig,
  recipient: recipient.publicKey,
  feeRecipient: feeRecipient.publicKey,
  signer: signer.publicKey,
  systemProgram: SystemProgram.programId,
}).signers([signer]).rpc();
```

---

## Debugging

### Enable Logs
```bash
export RUST_LOG=solana_runtime::system_instruction_processor=trace,solana_runtime::message_processor=trace,solana_bpf_loader=debug,solana_rbpf=debug
solana logs --url devnet
```

### Check Program Logs
```bash
solana logs <PROGRAM_ID> --url devnet
```

### View Account Data
```bash
solana account <BUCKET_ACCOUNT> --url devnet
```

---

## Common Issues

### Issue: "init_if_needed requires feature"
**Solution**: Add to Cargo.toml:
```toml
anchor-lang = { version = "0.29.0", features = ["init-if-needed"] }
```

### Issue: "Pod trait not implemented"
**Solution**: Use `#[account(zero_copy(unsafe))]` for large arrays

### Issue: "reference to packed field is unaligned"
**Solution**: Copy field to local variable first:
```rust
let max_deposit = tree_account.max_deposit_amount;
msg!("Max deposit: {}", max_deposit);
```

---

## Performance

### Current Test Times
```
test result: ok. 11 passed; 0 failed; finished in 0.00s
```

### Expected Transaction Times (on devnet)
- Initialize: ~2-3 seconds
- Deposit: ~1-2 seconds
- Withdraw: ~1-2 seconds

---

## Next Steps

Once unit tests pass, move to:

1. **Integration Tests**: Test with actual Solana transactions
2. **ZK Circuit**: Implement real withdrawal proof circuit
3. **Client SDK**: Build TypeScript client library
4. **End-to-End Tests**: Full flow from deposit to withdrawal
5. **Stress Tests**: Test with 100 deposits per bucket
6. **Security Audit**: Professional security review

---

## Useful Commands

```bash
# Run tests
cargo test -p pivy --lib

# Build program
cargo build-sbf -p pivy

# Check code
cargo clippy -p pivy

# Format code
cargo fmt -p pivy

# Clean
cargo clean -p pivy

# View docs
cargo doc -p pivy --open
```

---

## Test Data

### Example Commitment
```
amount: 1000
metaSpendPublic: [10, 10, 10, ..., 10] (32 bytes)
blinding: [42, 42, 42, ..., 42] (32 bytes)
commitment: hash(amount, metaSpendPublic, blinding)
```

### Example Bucket
```rust
BucketAccount {
    commitment_count: 2,
    total_balance: 3000,
    is_spent: false,
    commitments: [
        [1, 1, 1, ..., 1],  // John's deposit
        [2, 2, 2, ..., 2],  // Sarah's deposit
        [0, 0, 0, ..., 0],  // Empty
        ...
    ],
    bump: 254,
}
```

---

## Success Criteria

✅ All 11 tests pass
✅ No compilation warnings (except deprecations)
✅ Build succeeds with `cargo build-sbf`
✅ Program deploys to devnet
✅ Deposit instruction works
✅ Withdrawal instruction works
✅ Double-spend prevented

---

## Support

For issues or questions:
1. Check test output for specific error
2. Review error codes in `src/lib.rs`
3. Check Anchor logs
4. Verify account constraints

Happy testing! 🚀
