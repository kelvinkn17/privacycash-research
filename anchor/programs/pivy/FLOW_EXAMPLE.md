# PIVY Payment Flow Example

## Scenario: John pays Kelvin 1,000 USDC privately

### Step 1: Kelvin Creates PIVY Account

```bash
# Kelvin generates two keypairs locally
MetaView Keypair:
  - Private: [kept secret, never shared]
  - Public:  0xMETA_VIEW_KELVIN_123...

MetaSpend Keypair:
  - Private: [kept secret, never shared]
  - Public:  0xMETA_SPEND_KELVIN_456...

# Kelvin's pivy.me/kelvin page shows:
{
  "username": "kelvin",
  "metaViewPublic": "0xMETA_VIEW_KELVIN_123...",
  "metaSpendPublic": "0xMETA_SPEND_KELVIN_456...",
  "qrCode": "..." // QR with both public keys
}
```

**Important**: Kelvin NEVER shares his private keys!

---

### Step 2: John Wants to Pay Kelvin

```bash
# John visits pivy.me/kelvin
# John's client reads:
metaViewPublic = "0xMETA_VIEW_KELVIN_123..."
metaSpendPublic = "0xMETA_SPEND_KELVIN_456..."

# John enters amount: 1000 USDC
```

---

### Step 3: Client Generates Deposit Transaction

```javascript
// John's client generates commitment
const blinding = randomBytes(32); // Random secret
const commitment = hash(
  amount,              // 1000
  metaSpendPublic,     // Kelvin's public key
  blinding             // Random value
);
// Result: commitment = 0xABC123...

// John's client encrypts output for Kelvin
const encryptedOutput = encrypt(
  {
    amount: 1000,
    blinding: blinding,
    sender: "john" // optional
  },
  metaViewPublic      // Kelvin's MetaView public key
);
// Only Kelvin can decrypt this with MetaView private key!

// Generate blinded account ID for bucket PDA
const blindedAccountId = hash(metaSpendPublic);
```

---

### Step 4: John Submits Deposit

```rust
// John calls PIVY deposit instruction
deposit(
  commitment: 0xABC123...,
  encrypted_output: encryptedOutput,
  amount: 1000 * 10^6,  // 1000 USDC in smallest unit
  blinded_account_id: hash(metaSpendPublic)
)

// John signs with HIS wallet (not Kelvin's!)
// John transfers 1000 USDC from HIS wallet to PIVY pool
```

**What John DOES NOT have:**
- ❌ MetaView private key (can't decrypt)
- ❌ MetaSpend private key (can't withdraw)
- ❌ Kelvin's main wallet address
- ❌ Control over funds after deposit

**What's stored on-chain:**
```rust
BucketAccount {
  commitment_count: 1,
  total_balance: 1000,
  commitments: [0xABC123..., ...],
  is_spent: false
}

MerkleTree {
  // Commitment 0xABC123... added at index 42
}

Event::DepositEvent {
  commitment: 0xABC123...,
  encrypted_output: encryptedOutput,
  blinded_account_id: hash(metaSpendPublic)
}
```

---

### Step 5: Kelvin Checks Balance

```javascript
// Kelvin's client queries all deposit events
const events = await program.events.DepositEvent.all();

// Try to decrypt each one with MetaView private key
let balance = 0;
for (const event of events) {
  try {
    const decrypted = decrypt(
      event.encrypted_output,
      metaViewPrivate  // Kelvin's MetaView private key
    );

    if (decrypted) {
      balance += decrypted.amount;
      console.log(`Received ${decrypted.amount} from ${decrypted.sender}`);
    }
  } catch {
    // Not for Kelvin, skip
  }
}

console.log(`Total balance: ${balance}`); // Output: 1000
```

**Kelvin sees:**
- ✅ Balance: 1,000 USDC
- ✅ From: john (if included in metadata)
- ✅ Date: timestamp of deposit

---

### Step 6: Sarah Also Pays Kelvin

```bash
# Sarah visits pivy.me/kelvin
# Sarah deposits 2,000 USDC
# Same process as John

# On-chain state:
BucketAccount {
  commitment_count: 2,
  total_balance: 3000,  // Aggregated!
  commitments: [0xABC123..., 0xDEF456..., ...],
  is_spent: false
}
```

**Kelvin's balance now: 3,000 USDC** (1000 from John + 2000 from Sarah)

---

### Step 7: Kelvin Withdraws

```javascript
// Kelvin generates ZK proof
const proof = generateWithdrawalProof({
  metaSpendPrivate: metaSpendPrivate,    // Kelvin's private key
  bucketCommitments: [0xABC123..., 0xDEF456...],
  totalAmount: 3000,
  recipientAddress: kelvinMainWallet
});

// Kelvin calls withdraw instruction
withdraw(
  proof: proof,
  withdrawal_amount: 3000
);

// PIVY program:
// 1. Verifies ZK proof (Kelvin knows MetaSpend private key)
// 2. Checks bucket not already spent
// 3. Calculates fee: 3000 * 0.25% = 7.5 USDC
// 4. Transfers 3000 USDC to kelvinMainWallet
// 5. Transfers 7.5 USDC fee to fee recipient
// 6. Marks bucket as spent

BucketAccount {
  commitment_count: 2,
  total_balance: 0,        // Withdrawn
  is_spent: true,          // Prevent double-spend
  ...
}
```

**Result:**
- ✅ Kelvin receives 3,000 USDC in ONE transaction
- ✅ Fee: 7.5 USDC (0.25%)
- ✅ Time: ~1 second
- ✅ Privacy: On-chain, no link between John/Sarah deposits and Kelvin's withdrawal

---

## Comparison: Multiple Withdrawals

### Privacy-Cash Approach
```
10 deposits = 10 individual UTXOs

Withdraw all 10:
- Transaction 1: Withdraw UTXO 1 + 2 (2 minutes)
- Transaction 2: Withdraw UTXO 3 + 4 (2 minutes)
- Transaction 3: Withdraw UTXO 5 + 6 (2 minutes)
- Transaction 4: Withdraw UTXO 7 + 8 (2 minutes)
- Transaction 5: Withdraw UTXO 9 + 10 (2 minutes)

Total: 5 transactions, 10+ minutes
```

### PIVY Approach
```
10 deposits = 1 bucket with 10 commitments

Withdraw all 10:
- Transaction 1: Withdraw entire bucket (1 second)

Total: 1 transaction, ~1 second
```

**100x faster!** ⚡

---

## Security Properties

### What John Can't Do
- ❌ See Kelvin's balance (doesn't have MetaView private key)
- ❌ Withdraw Kelvin's funds (doesn't have MetaSpend private key)
- ❌ Know Kelvin's main wallet address
- ❌ Link Sarah's deposit to Kelvin

### What Kelvin Can't Do
- ❌ Withdraw twice (bucket marked as spent)
- ❌ Withdraw more than balance
- ❌ Skip the withdrawal fee

### What's Private
- ✅ Kelvin's main wallet address (not exposed)
- ✅ Link between John's deposit and Kelvin's withdrawal
- ✅ Link between Sarah's deposit and Kelvin's withdrawal
- ✅ Kelvin's total balance (encrypted)

### What's Public
- ❌ Deposit amounts (visible in events, but not linked to recipient)
- ❌ Withdrawal amount (visible on-chain)
- ❌ Commitment hashes (but meaningless without private keys)

---

## Key Insight

**John deposits using MetaSpend PUBLIC key**
**Kelvin withdraws using MetaSpend PRIVATE key**

John CAN'T withdraw (doesn't have private key)
Only Kelvin can withdraw (has private key)

The commitment `hash(amount, metaSpend_PUB, blinding)` ensures:
- John can create it with just the public key
- Only Kelvin can prove ownership with the private key
- No one else can claim the funds
