# PIVY Circuits

Zero-knowledge circuits for PIVY privacy protocol on Solana.

---

## 🚨 WHY IS THERE NO DEPOSIT CIRCUIT?

**Answer**: **Deposits DON'T need ZK proofs!**

When you deposit:
1. You send SOL to the pool (public transaction)
2. Program computes a commitment on-chain: `commitment = Poseidon(amount, metaSpendPubkey, blinding, mint)`
3. Commitment is stored in Merkle tree

**No ZK proof needed** - you're putting money IN, not taking it out.

**Only withdrawals need ZK proofs** - to prove you own the money without revealing which deposits are yours.

---

## Circuit Architecture

```
circuits/
├── keypair.circom              Helper: Derive pubkey from privkey
├── merkleProof.circom          Helper: Verify UTXO in Merkle tree
├── transaction.circom          MAIN: Universal transaction circuit
├── transaction_2x2.circom      Instantiation: 2 inputs, 2 outputs
└── bucket_withdrawal.circom    PIVY's innovation: Multi-deposit withdrawal
```

---

## 1. Universal Transaction Circuit (`transaction.circom`)

**Purpose**: Handles deposits, withdrawals, AND transfers with ONE circuit!

### How It Works

The circuit uses `publicAmount` to determine operation type:

**Deposit** (publicAmount > 0):
```
Inputs:  empty + empty
Public:  +5 SOL (deposit)
Outputs: 5 SOL UTXO + empty

Equation: 0 + 0 + 5 = 5 + 0 ✓
```

**Withdrawal** (publicAmount < 0):
```
Inputs:  5 SOL UTXO + empty
Public:  -5 SOL (withdraw)
Outputs: empty + empty

Equation: 5 + 0 - 5 = 0 + 0 ✓
```

**Transfer** (publicAmount = 0):
```
Inputs:  3 SOL UTXO + 2 SOL UTXO
Public:  0 SOL
Outputs: 5 SOL UTXO + empty

Equation: 3 + 2 + 0 = 5 + 0 ✓
```

### Circuit Parameters

- **Tree depth**: 26 levels (67M capacity)
- **Inputs**: 2 UTXOs max
- **Outputs**: 2 UTXOs max
- **Constraints**: ~50,000
- **Proof time**: ~7 seconds

### What It Proves

For each input UTXO:
1. ✅ You know the MetaSpend private key
2. ✅ Commitment = Poseidon(amount, pubkey, blinding, mint)
3. ✅ Commitment exists in Merkle tree
4. ✅ Nullifier is correctly computed

For each output UTXO:
1. ✅ Commitment is correctly computed
2. ✅ Amount doesn't overflow (248 bits max)

Global checks:
1. ✅ No duplicate nullifiers
2. ✅ Balance equation: inputs + publicAmount = outputs

---

## 2. Transaction 2x2 (`transaction_2x2.circom`)

**Purpose**: Instantiation of universal circuit with standard parameters.

Just wraps the main circuit:
```circom
component main {public [
    root,
    publicAmount,
    extDataHash,
    inputNullifier,
    outputCommitment
]} = Transaction(26, 2, 2);
```

**Use this for**:
- Standard deposits (create 1-2 UTXOs)
- Standard withdrawals (spend 1-2 UTXOs)
- UTXO consolidation
- UTXO splitting

---

## 3. Bucket Withdrawal (`bucket_withdrawal.circom`)

**Purpose**: PIVY's KEY INNOVATION - withdraw multiple deposits in ONE transaction!

### The Problem

Privacy Cash limitation:
```
Received 20 deposits → Must make 10 withdrawal transactions
Time: 10 tx × 7 seconds = 70 seconds
Cost: 10 × 0.0005 SOL = 0.005 SOL
```

### PIVY Solution

```
Received 20 deposits → Make 1 bucket withdrawal transaction
Time: 1 tx × 15 seconds = 15 seconds
Cost: 1 × 0.0005 SOL = 0.0005 SOL
```

**4.6x faster, 10x cheaper!**

### How It Works

1. User receives multiple deposits to same MetaSpend pubkey
2. Each deposit creates a commitment in Merkle tree
3. User proves ownership of ALL commitments with ONE MetaSpend private key
4. Withdraws sum of all deposits in single transaction

### Circuit Parameters

- **Tree depth**: 26 levels
- **Max deposits per bucket**: 20
- **Constraints**: ~100,000
- **Proof time**: ~15-20 seconds

### What It Proves

1. ✅ You know the MetaSpend private key
2. ✅ All commitments match this private key
3. ✅ Each commitment exists in Merkle tree
4. ✅ Sum of deposits = withdrawal amount
5. ✅ Bucket nullifier prevents double-spend

---

## Setup Instructions

### Prerequisites

```bash
# Install circom
npm install -g circom

# Install snarkjs
npm install -g snarkjs
```

### Step 1: Compile Circuits

```bash
./compile.sh
```

This creates:
- `transaction_2x2.r1cs` - Circuit constraints
- `transaction_2x2.wasm` - Witness generator
- `bucket_withdrawal.r1cs`
- `bucket_withdrawal.wasm`

### Step 2: Download Powers of Tau

```bash
./download_ptau.sh
```

Downloads the universal trusted setup (~600MB).

**Note**: This is the SAME Powers of Tau used by many ZK projects (Tornado Cash, Hermez, etc). It's already trusted by the community.

### Step 3: Generate Proving Keys

```bash
./setup_keys.sh
```

This creates:
- `transaction_2x2_final.zkey` - Proving key
- `transaction_2x2_vkey.json` - Verification key
- `bucket_withdrawal_final.zkey`
- `bucket_withdrawal_vkey.json`

**Takes ~5-10 minutes** depending on your machine.

---

## Usage Examples

### Example 1: Deposit (No Circuit Needed!)

```typescript
// Just call the Solana program directly
await program.methods
  .deposit(amount, commitment, encryptedOutput)
  .accounts({
    user: wallet.publicKey,
    pool: poolPDA,
    treeAccount: treePDA,
  })
  .rpc();

// Program computes commitment on-chain, no ZK proof!
```

### Example 2: Standard Withdrawal (transaction_2x2)

```typescript
import { groth16 } from "snarkjs";

// Prepare circuit inputs
const input = {
  // Public
  root: merkleTree.root(),
  publicAmount: -5_000_000_000, // -5 SOL
  extDataHash: computeExtDataHash(recipient, fee),
  inputNullifier: [nullifier1, nullifier2],
  outputCommitment: [commitment1, commitment2],

  // Private (never leaves client!)
  inAmount: [5_000_000_000, 0],
  inPrivateKey: [metaSpendPrivate, 0],
  inBlinding: [blinding1, 0],
  inPathIndices: [pathIndex1, 0],
  inPathElements: [pathElements1, emptyPath],

  outAmount: [0, 0],
  outPubkey: [0, 0],
  outBlinding: [randomBlinding1, randomBlinding2],

  mintAddress: "11111111111111111111111111111112" // SOL
};

// Generate proof (~7 seconds)
const { proof, publicSignals } = await groth16.fullProve(
  input,
  "transaction_2x2.wasm",
  "transaction_2x2_final.zkey"
);

// Submit to Solana
await program.methods
  .withdraw(proof, extData, encryptedOutputs)
  .accounts({ ... })
  .rpc();
```

### Example 3: Bucket Withdrawal (bucket_withdrawal)

```typescript
// User received 10 deposits, withdraws all at once!

const input = {
  // Public
  root: merkleTree.root(),
  metaSpendPublic: myMetaSpendPubkey,
  totalWithdrawal: 50_000_000_000, // 50 SOL total
  extDataHash: computeExtDataHash(recipient, fee),
  mintAddress: "11111111111111111111111111111112",

  // Private
  metaSpendPrivate: myMetaSpendPrivate,
  depositCount: 10,
  amounts: [5e9, 5e9, 5e9, ...], // 10 deposits of 5 SOL each
  blindings: [blinding1, blinding2, ...],
  pathIndices: [index1, index2, ...],
  pathElements: [path1, path2, ...],
};

// Generate proof (~15-20 seconds)
const { proof, publicSignals } = await groth16.fullProve(
  input,
  "bucket_withdrawal.wasm",
  "bucket_withdrawal_final.zkey"
);

// Submit to Solana
await program.methods
  .bucketWithdraw(proof, extData)
  .accounts({ ... })
  .rpc();
```

---

## Circuit Comparison

| Feature | transaction_2x2 | bucket_withdrawal |
|---------|----------------|-------------------|
| **Purpose** | Standard ops | Multi-deposit withdrawal |
| **Max inputs** | 2 UTXOs | 20 deposits |
| **Constraints** | ~50k | ~100k |
| **Proof time** | ~7s | ~15-20s |
| **Use when** | <5 deposits | 5-20 deposits |
| **Cost** | Low | Medium |
| **Speed** | Fast (per tx) | Very fast (total) |

---

## Performance Benchmarks

### Scenario: User received 20 deposits

**Option 1: Standard withdrawals (transaction_2x2)**
```
Transactions: 10 (20 deposits ÷ 2 inputs per tx)
Proof time: 10 × 7s = 70 seconds
Network time: 10 × 3s = 30 seconds
Total: ~100 seconds
Fees: 10 × 0.0005 SOL = 0.005 SOL
```

**Option 2: Bucket withdrawal (bucket_withdrawal)**
```
Transactions: 1
Proof time: 15 seconds
Network time: 3 seconds
Total: ~18 seconds
Fees: 1 × 0.0005 SOL = 0.0005 SOL
```

**Improvement**: 5.5x faster, 10x cheaper!

---

## Security Considerations

### 1. Trusted Setup

Both circuits use Groth16, which requires a trusted setup:
- We use the universal Powers of Tau ceremony
- Same ceremony used by Tornado Cash, Hermez, etc
- Community has contributed randomness

**For production**: Consider running your own contribution phase.

### 2. Circuit Audits

These circuits should be audited before mainnet deployment:
- Check for under-constrained circuits
- Verify nullifier uniqueness
- Test edge cases (zero amounts, max values, etc)

### 3. Key Management

- **MetaSpend private key**: NEVER send to backend or expose online
- **Proving keys (.zkey)**: Can be public
- **Verification keys**: Stored on-chain in Solana program

---

## Troubleshooting

### "Cannot find circomlib"

```bash
# Install circomlib in node_modules
cd ../../../
npm install circomlib
```

### "Powers of Tau file not found"

```bash
# Run the download script
./download_ptau.sh
```

### "Compilation fails"

```bash
# Check circom version
circom --version
# Should be 2.0.0 or higher

# Try with more memory
NODE_OPTIONS="--max-old-space-size=4096" ./compile.sh
```

### "Proof generation too slow"

- Use a faster machine (more CPU cores)
- Consider reducing `maxDeposits` in bucket_withdrawal
- Implement Web Workers for parallel proving

---

## Next Steps

1. ✅ Compile circuits: `./compile.sh`
2. ✅ Download Powers of Tau: `./download_ptau.sh`
3. ✅ Generate keys: `./setup_keys.sh`
4. 🔄 Integrate with Solana program (see `../src/lib.rs`)
5. 🔄 Write tests (see `../tests/`)
6. 🔄 Deploy to devnet
7. 🔄 Get audit
8. 🔄 Deploy to mainnet

---

## Resources

- **Circom Documentation**: https://docs.circom.io/
- **snarkjs Documentation**: https://github.com/iden3/snarkjs
- **Tornado Cash Circuits**: https://github.com/tornadocash/tornado-core
- **Light Protocol**: https://github.com/Lightprotocol/light-protocol

---

## FAQ

**Q: Why no deposit circuit?**
A: Deposits don't need proofs - you're adding money, not taking it out!

**Q: Can I have more than 2 inputs in transaction_2x2?**
A: No, it's hardcoded to 2. For more, use bucket_withdrawal or make multiple transactions.

**Q: What's the max deposits in bucket_withdrawal?**
A: Currently 20. You can increase it, but proof time scales linearly (~0.75s per deposit).

**Q: Is this compatible with Privacy Cash?**
A: transaction_2x2 is compatible. bucket_withdrawal is PIVY-specific.

**Q: Do I need a new trusted setup for each circuit?**
A: No! Same Powers of Tau works for all circuits. Just the .zkey files differ.

**Q: Can I use different elliptic curves?**
A: No, these circuits use BN254 (alt_bn128). Changing requires rewriting everything.

---

## License

MIT License - see LICENSE file for details.
