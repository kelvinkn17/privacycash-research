# Privacy Cash: Balance Retrieval & Transaction Indexing Deep Dive

**Date**: October 19, 2025
**Analyzed by**: Claude Code

---

## Executive Summary

**Your Questions**:
1. How does Privacy Cash get your balance in the pool?
2. Is it using signatures or something else?
3. Can Privacy Cash index transactions (list of deposits/withdrawals)?

**Answers**:
1. ✅ **Balance via Event Scanning + Decryption** (NOT signatures!)
2. ✅ **Uses encrypted_output in events** (decrypt with your private key)
3. ✅ **Full indexing possible** via Solana event logs and program accounts

---

## Part 1: How Balance Retrieval Works

### The Core Challenge

On-chain, everything looks like random hashes:
```
Commitment #1: 0x7a4f8e92...bc31 (random 32 bytes)
Commitment #2: 0x1c9e2d3a...ef89 (random 32 bytes)
Commitment #3: 0x5b3f7c1e...4a92 (random 32 bytes)

Question: Which ones are YOURS?
Answer: You can't tell by looking at commitments alone!
```

**Problem**: Commitments are designed to be unlinkable and private. But this means even YOU can't tell which ones you own without additional information.

**Solution**: `encrypted_output` in `CommitmentData` events!

**Code Reference**: `lib.rs:254-275` - CommitmentData event structure

---

## The encrypted_output Mechanism

### On-Chain Event Structure

Every transaction emits **2 CommitmentData events**:

```rust
// lib.rs:270-275
#[event]
pub struct CommitmentData {
    pub index: u64,                  // Position in Merkle tree
    pub commitment: [u8; 32],        // Random-looking hash
    pub encrypted_output: Vec<u8>,   // 👈 THIS is the key!
}

// lib.rs:254-264
emit!(CommitmentData {
    index: next_index_to_insert,
    commitment: proof.output_commitments[0],
    encrypted_output: encrypted_output1.to_vec(),  // Contains UTXO details
});

emit!(CommitmentData {
    index: second_index,
    commitment: proof.output_commitments[1],
    encrypted_output: encrypted_output2.to_vec(),
});
```

**What's in `encrypted_output`?**

```typescript
// Before encryption
const utxoData = {
  amount: 5_000_000_000,           // 5 SOL in lamports
  pubkey: "0x7a4f...bc31",         // Owner's public key
  blinding: "0x1c9e...ef89",       // Random blinding factor
  index: 42,                        // Position in tree
  mintAddress: "11111...1112"      // SOL mint
};

// Encrypted with recipient's public key
const encrypted_output = encrypt(
  recipientPublicKey,  // Only recipient can decrypt
  JSON.stringify(utxoData)
);
```

**Code Reference**: `anchor/tests/zkcash.ts:890-926` - Event parsing example

---

## Step-by-Step: How to Get Your Balance

### Method 1: Client-Side Scanning (Privacy-Preserving)

```typescript
// 1. Fetch all CommitmentData events from the program
const events = await program.account.commitmentData.all();

// 2. Try to decrypt each one with YOUR private key
let myUTXOs = [];
let totalBalance = 0;

for (const event of events) {
  try {
    // Attempt decryption
    const decrypted = decrypt(
      myPrivateKey,
      event.data.encryptedOutput
    );

    // If decryption succeeds, this UTXO is yours!
    const utxo = JSON.parse(decrypted);
    myUTXOs.push({
      amount: utxo.amount,
      commitment: event.data.commitment,
      index: event.data.index,
      blinding: utxo.blinding,
      pubkey: utxo.pubkey
    });

    totalBalance += utxo.amount;
  } catch (e) {
    // Decryption failed - not your UTXO, skip it
    continue;
  }
}

// 3. Check which UTXOs are already spent (nullified)
const unspentUTXOs = myUTXOs.filter(utxo => {
  const nullifier = computeNullifier(utxo);
  const nullifierPDA = deriveNullifierPDA(nullifier);

  // Check if nullifier account exists on-chain
  return !accountExists(nullifierPDA);  // If doesn't exist, unspent
});

// 4. Calculate final balance
const balance = unspentUTXOs.reduce((sum, utxo) => sum + utxo.amount, 0);
console.log(`Your balance: ${balance / LAMPORTS_PER_SOL} SOL`);
```

**Key Insight**:
- ❌ NOT using signatures to verify ownership
- ✅ Using **decryption** to discover UTXOs
- ✅ Only you can decrypt your `encrypted_output`
- ✅ Fully private - no one knows what you're scanning for

**Code Reference**: `anchor/tests/zkcash.ts:890-926` - Event parsing

---

### Method 2: Backend-Assisted (Faster, But Less Private)

```typescript
// Backend continuously scans and decrypts for ALL users
class PrivacyCashBackend {
  async scanForUser(userPubkey: string) {
    // 1. Backend has indexed all events in database
    const allEvents = await db.events.findAll();

    // 2. Backend tries to decrypt with user's public key
    //    (User must have shared viewing key or backend stores it)
    const userUTXOs = [];

    for (const event of allEvents) {
      try {
        const decrypted = decrypt(
          userPubkey,  // Or user's viewing key
          event.encrypted_output
        );

        userUTXOs.push({
          ...JSON.parse(decrypted),
          commitment: event.commitment,
          index: event.index,
          blockHeight: event.blockHeight,
          timestamp: event.timestamp
        });
      } catch {
        continue;
      }
    }

    // 3. Filter out spent UTXOs
    const unspent = await this.filterUnspent(userUTXOs);

    return {
      utxos: unspent,
      balance: unspent.reduce((sum, u) => sum + u.amount, 0)
    };
  }

  async filterUnspent(utxos) {
    return utxos.filter(async utxo => {
      const nullifier = computeNullifier(utxo);
      const nullifierPDA = deriveNullifierPDA(nullifier);

      // Check on-chain if nullifier exists
      const account = await connection.getAccountInfo(nullifierPDA);
      return account === null;  // null = unspent
    });
  }
}

// User calls backend API
const response = await fetch('/api/balance', {
  method: 'POST',
  body: JSON.stringify({
    pubkey: myPublicKey,
    // Optional: provide viewing key separately from spend key
    viewingKey: myViewingKey
  })
});

const { balance, utxos } = await response.json();
console.log(`Balance: ${balance} SOL`);
console.log(`UTXOs: ${utxos.length}`);
```

**Trade-offs**:
- ✅ **Much faster**: Backend pre-indexes everything
- ✅ **Better UX**: Instant balance updates
- ⚠️ **Less private**: Backend knows your UTXOs
- ⚠️ **Trust required**: Backend could log/leak data

**Code Reference**: `lib.rs:254-264` - Events backend would scan

---

## Part 2: Nullifier Checking (Determining Spent vs Unspent)

### How to Know if a UTXO is Spent

```typescript
// After discovering your UTXOs, check if they're spent

async function isUTXOSpent(utxo: UTXO): Promise<boolean> {
  // 1. Compute nullifier from UTXO data
  const commitment = await utxo.getCommitment();
  const signature = utxo.keypair.sign(commitment, utxo.index.toString());
  const nullifier = poseidonHash([
    commitment,
    utxo.index.toString(),
    signature
  ]);

  // 2. Derive nullifier PDA (Program Derived Address)
  const [nullifierPDA] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("nullifier0"),  // or "nullifier1"
      Buffer.from(nullifier)
    ],
    programId
  );

  // 3. Check if account exists on-chain
  const account = await connection.getAccountInfo(nullifierPDA);

  // If account exists, UTXO has been spent
  // If null, UTXO is still unspent
  return account !== null;
}

// Usage
const myUTXOs = await discoverMyUTXOs();
const unspentUTXOs = [];

for (const utxo of myUTXOs) {
  const spent = await isUTXOSpent(utxo);
  if (!spent) {
    unspentUTXOs.push(utxo);
  }
}

const balance = unspentUTXOs.reduce((sum, u) => sum + u.amount, 0);
```

**Why This Works**:
- When you spend a UTXO, the program creates a nullifier account
- Nullifier accounts are permanent (cannot be deleted)
- Checking account existence = checking if spent

**Code Reference**: `lib.rs:329-364` - Nullifier account creation

---

## Part 3: Transaction Indexing Capabilities

### Question: Can Privacy Cash Index All Transactions?

**Answer**: ✅ **YES, fully indexable!**

Privacy Cash emits **on-chain events** for every transaction, making it fully auditable.

---

## What Can Be Indexed?

### 1. All Deposits

```typescript
interface DepositEvent {
  type: "CommitmentData",
  blockHeight: number,
  timestamp: number,
  txSignature: string,
  data: {
    index: number,              // Position in Merkle tree
    commitment: string,         // 0x7a4f... (random hash)
    encryptedOutput: Buffer,    // Encrypted UTXO data
  }
}

// Example indexed deposit
{
  type: "deposit",
  blockHeight: 123456,
  timestamp: 1697123456,
  txSignature: "5Kw8...",
  commitment: "0x7a4f...",
  index: 42,
  // NOTE: Cannot tell WHO deposited or HOW MUCH (encrypted!)
}
```

**What's Visible**:
- ✅ Block height and timestamp
- ✅ Transaction signature
- ✅ Commitment hash (but meaningless to outsiders)
- ✅ Tree index position
- ❌ **NOT visible**: Amount, sender, recipient

---

### 2. All Withdrawals

```typescript
interface WithdrawalEvent {
  type: "transaction",
  blockHeight: number,
  timestamp: number,
  txSignature: string,
  data: {
    inputNullifiers: [string, string],     // Spent UTXOs (hashes)
    outputCommitments: [string, string],   // New UTXOs created
    publicAmount: string,                  // Withdrawal amount (visible!)
    recipient: PublicKey,                  // Recipient address (visible!)
    extDataHash: string,
  }
}

// Example indexed withdrawal
{
  type: "withdrawal",
  blockHeight: 123789,
  timestamp: 1697123789,
  txSignature: "9Xm2...",
  nullifiers: ["0x5b3f...", "0x1c9e..."],
  newCommitments: ["0x8a2d...", "0x4f7b..."],
  withdrawalAmount: 5000000000,  // 5 SOL (VISIBLE!)
  recipient: "7kNq...xY3z",      // Public address (VISIBLE!)
}
```

**What's Visible**:
- ✅ Block height and timestamp
- ✅ Transaction signature
- ✅ **Withdrawal amount** (public!)
- ✅ **Recipient address** (public!)
- ✅ Nullifiers (but can't link to deposits without private key)
- ❌ **NOT visible**: Which deposits were spent (unlinkable!)

**Code Reference**: `lib.rs:254-264` - CommitmentData events

---

## Full Transaction History API Design

### Backend API for Transaction History

```typescript
// API: GET /api/transactions/:address

interface TransactionHistory {
  address: string;
  deposits: Deposit[];
  withdrawals: Withdrawal[];
  balance: number;
}

interface Deposit {
  txSignature: string;
  blockHeight: number;
  timestamp: number;
  amount: number;              // Only known if you can decrypt
  commitment: string;
  index: number;
  status: "unspent" | "spent";
}

interface Withdrawal {
  txSignature: string;
  blockHeight: number;
  timestamp: number;
  amount: number;              // Public (visible to all)
  recipient: string;           // Public
  nullifiers: string[];        // Which UTXOs were spent
}

// Implementation
class TransactionIndexer {
  async getTransactionHistory(
    userPubkey: string,
    viewingKey: string
  ): Promise<TransactionHistory> {

    // 1. Scan all CommitmentData events
    const allEvents = await this.getAllCommitmentEvents();

    // 2. Decrypt to find user's deposits
    const deposits = [];
    for (const event of allEvents) {
      try {
        const decrypted = decrypt(viewingKey, event.data.encryptedOutput);
        const utxo = JSON.parse(decrypted);

        // Check if spent
        const nullifier = computeNullifier(utxo);
        const spent = await this.isNullifierUsed(nullifier);

        deposits.push({
          txSignature: event.txSignature,
          blockHeight: event.blockHeight,
          timestamp: event.timestamp,
          amount: utxo.amount,
          commitment: event.data.commitment,
          index: event.data.index,
          status: spent ? "spent" : "unspent"
        });
      } catch {
        continue;  // Not user's UTXO
      }
    }

    // 3. Find withdrawals by scanning nullifiers
    const withdrawals = [];
    for (const deposit of deposits.filter(d => d.status === "spent")) {
      // Find which transaction spent this UTXO
      const spendTx = await this.findSpendTransaction(deposit.commitment);

      withdrawals.push({
        txSignature: spendTx.signature,
        blockHeight: spendTx.blockHeight,
        timestamp: spendTx.timestamp,
        amount: spendTx.publicAmount,
        recipient: spendTx.recipient,
        nullifiers: spendTx.nullifiers
      });
    }

    // 4. Calculate balance
    const unspentDeposits = deposits.filter(d => d.status === "unspent");
    const balance = unspentDeposits.reduce((sum, d) => sum + d.amount, 0);

    return { address: userPubkey, deposits, withdrawals, balance };
  }

  async getAllCommitmentEvents() {
    // Fetch from Solana using getProgramAccounts or RPC getLogs
    const signatures = await connection.getSignaturesForAddress(programId);

    const events = [];
    for (const sig of signatures) {
      const tx = await connection.getTransaction(sig.signature, {
        maxSupportedTransactionVersion: 0
      });

      if (tx?.meta?.logMessages) {
        const parser = new EventParser(programId, new BorshCoder(idl));
        const parsed = Array.from(parser.parseLogs(tx.meta.logMessages));

        for (const event of parsed) {
          if (event.name === "commitmentData") {
            events.push({
              txSignature: sig.signature,
              blockHeight: tx.slot,
              timestamp: tx.blockTime,
              data: event.data
            });
          }
        }
      }
    }

    return events;
  }

  async isNullifierUsed(nullifier: string): Promise<boolean> {
    const [nullifierPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier0"), Buffer.from(nullifier)],
      programId
    );

    const account = await connection.getAccountInfo(nullifierPDA);
    return account !== null;
  }
}
```

**Code Reference**:
- `anchor/tests/zkcash.ts:883-927` - Event parsing
- `lib.rs:329-364` - Nullifier account structure

---

## Privacy Levels: What Can Different Parties See?

### Public Observer (No Keys)

```typescript
// What ANYONE can see on-chain
{
  commitments: [
    "0x7a4f...",  // Meaningless hash
    "0x1c9e...",  // Meaningless hash
    "0x5b3f...",  // Meaningless hash
  ],
  withdrawals: [
    {
      amount: 5_000_000_000,      // 5 SOL (VISIBLE!)
      recipient: "7kNq...xY3z",   // Public address (VISIBLE!)
      nullifiers: ["0x5b3f..."]   // But can't link to deposit!
    }
  ]
}
```

**Can Answer**:
- ✅ Total number of deposits ever made
- ✅ Total withdrawal amounts and recipients
- ✅ Pool's total value locked (TVL)
- ❌ **CANNOT** link deposits to withdrawals
- ❌ **CANNOT** see deposit amounts
- ❌ **CANNOT** identify depositors

---

### User (Has Private Key)

```typescript
// What YOU can see with your private key
{
  myDeposits: [
    { amount: 5_000_000_000, status: "unspent", timestamp: 1697123456 },
    { amount: 2_000_000_000, status: "spent", timestamp: 1697123789 },
  ],
  myWithdrawals: [
    { amount: 2_000_000_000, recipient: "my_wallet", timestamp: 1697124000 }
  ],
  balance: 5_000_000_000  // 5 SOL unspent
}
```

**Can Answer**:
- ✅ All YOUR deposits (amount, time, status)
- ✅ All YOUR withdrawals
- ✅ YOUR current balance
- ✅ YOUR transaction history
- ❌ **CANNOT** see other users' deposits
- ❌ **CANNOT** see other users' balances

---

### Backend (Has All Users' Viewing Keys)

```typescript
// What backend can see if users trust it with viewing keys
{
  totalUsers: 1234,
  totalDeposits: 5678,
  totalBalance: 123_000_000_000,  // 123 SOL
  userBalances: {
    "user1": 5_000_000_000,
    "user2": 10_000_000_000,
    // ... all users
  },
  allTransactions: [
    // Complete transaction graph for all users
  ]
}
```

**Can Answer**:
- ✅ Every user's balance
- ✅ Every user's transaction history
- ✅ Link deposits to withdrawals
- ✅ Generate compliance reports
- ⚠️ **PRIVACY RISK**: Backend is trusted party!

---

### Regulator (With Court Order + Backend Cooperation)

Same as backend, but only for **specific users** under investigation.

```typescript
// Court order: "Provide transaction history for user 0x7kNq...xY3z"

{
  user: "0x7kNq...xY3z",
  deposits: [
    { amount: 5 SOL, timestamp: "2024-10-15", source: "Unknown" },
    { amount: 10 SOL, timestamp: "2024-10-20", source: "Unknown" },
  ],
  withdrawals: [
    { amount: 5 SOL, timestamp: "2024-10-25", destination: "0xABC..." }
  ],
  currentBalance: 10 SOL
}
```

**Note**: Even with court order, **cannot** identify who sent the deposits (privacy preserved).

---

## Comparison: Privacy Cash vs Traditional Blockchain

### Traditional Solana Transfer

```typescript
// Everyone can see EVERYTHING
{
  from: "0x7kNq...xY3z",         // VISIBLE
  to: "0xABC...123",             // VISIBLE
  amount: 5_000_000_000,         // VISIBLE
  timestamp: 1697123456,         // VISIBLE
  balance_before: 10_000_000_000,// VISIBLE
  balance_after: 5_000_000_000,  // VISIBLE
}
```

**Privacy**: ❌ NONE

---

### Privacy Cash

```typescript
// Public view (no keys)
{
  commitment: "0x7a4f...",       // MEANINGLESS HASH
  encrypted_output: "0xAB12...", // GIBBERISH
  nullifier: "0x5b3f...",        // MEANINGLESS HASH
  withdrawal_amount: 5 SOL,      // VISIBLE (but can't link to deposit!)
  withdrawal_recipient: "0xABC", // VISIBLE
}

// User view (with private key)
{
  my_deposits: [
    { amount: 5 SOL, commitment: "0x7a4f...", status: "unspent" }
  ],
  my_balance: 5 SOL
}
```

**Privacy**:
- ✅ Deposit amounts hidden
- ✅ Deposit senders hidden
- ✅ Cannot link deposits to withdrawals
- ⚠️ Withdrawal amounts visible (but source hidden)

---

## Performance Analysis

### Client-Side Scanning (Privacy-Preserving)

```typescript
// Scanning 10,000 commitments

Step 1: Fetch all CommitmentData events
  - RPC calls: ~100 (batch requests)
  - Time: ~30-60 seconds
  - Data: ~10 MB

Step 2: Decrypt each event (try with your key)
  - Decryption attempts: 10,000
  - Time per attempt: ~0.2 ms
  - Total time: ~2 seconds

Step 3: Check nullifiers for unspent UTXOs
  - RPC calls: ~10 (your UTXOs only)
  - Time: ~1-2 seconds

Total time: ~33-64 seconds
```

**Trade-offs**:
- ✅ **Maximum privacy**: No one knows what you're looking for
- ✅ **No trust needed**: Fully client-side
- ❌ **Slow**: Must scan all events
- ❌ **Bandwidth**: Must download all events

**Code Reference**: `anchor/tests/zkcash.ts:883-927`

---

### Backend-Assisted (Faster, Less Private)

```typescript
// Backend pre-indexes everything

Step 1: Backend scans all events (done continuously)
  - Time: 0 seconds (already cached)

Step 2: Backend decrypts for you (or you provide viewing key)
  - Time: ~100 ms (database query)

Step 3: Backend checks nullifiers
  - Time: ~100 ms (database query)

Total time: ~200 ms
```

**Trade-offs**:
- ✅ **Fast**: Sub-second response
- ✅ **Good UX**: Instant balance updates
- ⚠️ **Less private**: Backend knows your UTXOs
- ⚠️ **Trust required**: Backend could log/leak

---

## Real-World Implementation: Privacy Cash SDK

### How Official SDK Likely Works

```typescript
// Pseudocode based on architecture

class PrivacyCashClient {
  constructor(
    private connection: Connection,
    private program: Program,
    private keypair: Keypair
  ) {}

  async getBalance(): Promise<number> {
    // Scan for UTXOs
    const utxos = await this.scanUTXOs();

    // Filter unspent
    const unspent = await this.filterUnspent(utxos);

    // Sum amounts
    return unspent.reduce((sum, u) => sum + u.amount, 0);
  }

  async scanUTXOs(): Promise<UTXO[]> {
    // Fetch all CommitmentData events
    const events = await this.fetchAllCommitmentEvents();

    // Try to decrypt each one
    const myUTXOs = [];
    for (const event of events) {
      try {
        const decrypted = this.decrypt(
          this.keypair.publicKey,
          event.data.encryptedOutput
        );

        myUTXOs.push({
          ...JSON.parse(decrypted),
          commitment: event.data.commitment,
          index: event.data.index
        });
      } catch {
        continue;
      }
    }

    return myUTXOs;
  }

  async filterUnspent(utxos: UTXO[]): Promise<UTXO[]> {
    return utxos.filter(async utxo => {
      const nullifier = await this.computeNullifier(utxo);
      const spent = await this.isNullifierUsed(nullifier);
      return !spent;
    });
  }

  async getTransactionHistory(): Promise<Transaction[]> {
    const utxos = await this.scanUTXOs();

    const history = [];

    // Add all deposits
    for (const utxo of utxos) {
      const spent = await this.isUTXOSpent(utxo);
      history.push({
        type: "deposit",
        amount: utxo.amount,
        status: spent ? "spent" : "unspent",
        commitment: utxo.commitment,
        index: utxo.index
      });
    }

    // Find withdrawals
    for (const utxo of utxos.filter(u => u.spent)) {
      const spendTx = await this.findSpendTransaction(utxo);
      history.push({
        type: "withdrawal",
        amount: spendTx.publicAmount,
        recipient: spendTx.recipient,
        timestamp: spendTx.timestamp
      });
    }

    return history;
  }
}
```

**Code Reference**: See official SDK at https://github.com/Privacy-Cash/privacy-cash-sdk

---

## Advanced Indexing: Building a Block Explorer

### What a Privacy Cash Block Explorer Can Show

```typescript
interface PrivacyCashExplorer {
  // Global stats (no privacy violated)
  stats: {
    totalCommitments: 67_543_210,
    totalWithdrawals: 12_345,
    tvl: 123_456_789_000,  // 123,456 SOL
    averageDepositSize: "Unknown",  // Encrypted!
  },

  // Recent activity (limited info)
  recentDeposits: [
    {
      commitment: "0x7a4f...",
      index: 42,
      timestamp: 1697123456,
      amount: "Unknown",  // Encrypted!
    }
  ],

  recentWithdrawals: [
    {
      amount: 5_000_000_000,      // 5 SOL (VISIBLE!)
      recipient: "7kNq...xY3z",   // VISIBLE!
      timestamp: 1697123789,
      nullifiers: ["0x5b3f..."],  // But can't link!
    }
  ],

  // Anonymity set size
  anonymitySet: {
    size: 67_543_210,  // All commitments in tree
    growth: "+234 last 24h"
  }
}
```

**What Block Explorer CANNOT Show**:
- ❌ Deposit amounts
- ❌ Depositor addresses
- ❌ Links between deposits and withdrawals
- ❌ User balances
- ❌ Transaction patterns

**Code Reference**: `lib.rs:254-264` - Public events

---

## Security Considerations

### Attack: Can Someone Spy on My Balance?

**Scenario**: Attacker wants to know your balance

**What They Can Do**:
1. ❌ **CANNOT** see your balance on-chain (commitments are hashes)
2. ❌ **CANNOT** decrypt `encrypted_output` without your private key
3. ❌ **CANNOT** link your deposits to you (no on-chain connection)

**What They Need**:
- ✅ Your private key (or viewing key if you shared it)
- ✅ Backend cooperation (if you use backend)

**Verdict**: **Secure against casual observers**

---

### Attack: Can Backend Leak My Data?

**Scenario**: Backend is compromised

**What Attacker Gets**:
- ✅ Your balance (if backend stores it)
- ✅ Your transaction history (if backend indexed it)
- ✅ Your UTXOs (if backend decrypts for you)

**What Attacker CANNOT Get**:
- ❌ Your private spending key (never sent to backend)
- ❌ Other users' data (unless backend stores all)

**Mitigation**:
- Use client-side scanning (no backend trust)
- Use view-only keys (separate from spending keys)
- Backend uses SGX enclaves for decryption

**Verdict**: **Backend is trust assumption** - choose wisely!

---

## Code Implementation Examples

### Example 1: Simple Balance Checker

```typescript
import { Connection, PublicKey } from "@solana/web3.js";
import { Program, EventParser, BorshCoder } from "@coral-xyz/anchor";
import { Keypair } from "./keypair";

async function getMyBalance(
  connection: Connection,
  program: Program,
  myKeypair: Keypair
): Promise<number> {

  // 1. Fetch all program transactions
  const signatures = await connection.getSignaturesForAddress(
    program.programId,
    { limit: 10000 }  // Adjust as needed
  );

  // 2. Parse all CommitmentData events
  const myUTXOs = [];

  for (const sig of signatures) {
    const tx = await connection.getTransaction(sig.signature, {
      maxSupportedTransactionVersion: 0
    });

    if (tx?.meta?.logMessages) {
      const parser = new EventParser(
        program.programId,
        new BorshCoder(program.idl)
      );
      const events = Array.from(parser.parseLogs(tx.meta.logMessages));

      for (const event of events) {
        if (event.name === "commitmentData") {
          try {
            // Try to decrypt
            const decrypted = decrypt(
              myKeypair.privateKey,
              event.data.encryptedOutput
            );

            // If successful, it's my UTXO!
            myUTXOs.push({
              ...JSON.parse(decrypted),
              commitment: event.data.commitment,
              index: event.data.index
            });
          } catch {
            // Not my UTXO, skip
          }
        }
      }
    }
  }

  // 3. Filter unspent UTXOs
  const unspent = [];
  for (const utxo of myUTXOs) {
    const nullifier = await computeNullifier(utxo);
    const [nullifierPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier0"), Buffer.from(nullifier)],
      program.programId
    );

    const account = await connection.getAccountInfo(nullifierPDA);
    if (account === null) {
      // Unspent!
      unspent.push(utxo);
    }
  }

  // 4. Sum up balance
  const balance = unspent.reduce((sum, u) => sum + u.amount, 0);
  return balance;
}
```

**Code Reference**: `anchor/tests/zkcash.ts:883-927`

---

### Example 2: Transaction History API

```typescript
import express from 'express';

const app = express();

// GET /api/history/:pubkey
app.get('/api/history/:pubkey', async (req, res) => {
  const { pubkey } = req.params;
  const { viewingKey } = req.query;

  if (!viewingKey) {
    return res.status(400).json({ error: "Viewing key required" });
  }

  // Scan for user's transactions
  const history = await getTransactionHistory(pubkey, viewingKey);

  res.json(history);
});

async function getTransactionHistory(
  pubkey: string,
  viewingKey: string
): Promise<TransactionHistory> {

  // 1. Scan all events (use cached version)
  const allEvents = await db.events.findAll();

  // 2. Decrypt to find user's deposits
  const deposits = [];
  for (const event of allEvents) {
    try {
      const decrypted = decrypt(viewingKey, event.encrypted_output);
      const utxo = JSON.parse(decrypted);

      // Check if spent
      const spent = await isNullifierUsed(utxo);

      deposits.push({
        type: "deposit",
        txSignature: event.tx_signature,
        blockHeight: event.block_height,
        timestamp: event.timestamp,
        amount: utxo.amount,
        commitment: event.commitment,
        status: spent ? "spent" : "unspent"
      });
    } catch {
      continue;
    }
  }

  // 3. Find withdrawals
  const withdrawals = [];
  for (const deposit of deposits.filter(d => d.status === "spent")) {
    const spendTx = await db.transactions.findOne({
      where: { nullifiers: { contains: deposit.nullifier } }
    });

    if (spendTx) {
      withdrawals.push({
        type: "withdrawal",
        txSignature: spendTx.signature,
        blockHeight: spendTx.block_height,
        timestamp: spendTx.timestamp,
        amount: spendTx.public_amount,
        recipient: spendTx.recipient
      });
    }
  }

  // 4. Calculate balance
  const unspent = deposits.filter(d => d.status === "unspent");
  const balance = unspent.reduce((sum, d) => sum + d.amount, 0);

  return {
    pubkey,
    deposits,
    withdrawals,
    balance,
    lastUpdated: Date.now()
  };
}
```

---

## Summary

### How Balance Retrieval Works

1. **Event Scanning**: Scan all `CommitmentData` events from blockchain
2. **Decryption**: Try to decrypt `encrypted_output` with your private key
3. **Discovery**: Successfully decrypted events = your UTXOs
4. **Nullifier Check**: Check if UTXOs are spent (nullifier PDA exists)
5. **Balance Calc**: Sum all unspent UTXOs

**Key Point**: ❌ **NOT** using signatures, ✅ using **decryption** to discover ownership!

---

### Transaction Indexing Capabilities

✅ **Fully indexable**:
- All commitments (with encrypted metadata)
- All withdrawals (with public amounts/recipients)
- All nullifiers (spent UTXOs)
- Block heights and timestamps

⚠️ **Privacy preserved**:
- Deposit amounts hidden
- Depositors hidden
- Cannot link deposits to withdrawals

---

### Privacy Model

| Party | Can See |
|-------|---------|
| **Public Observer** | Commitments (hashes), withdrawal amounts/recipients, TVL |
| **User (You)** | Your deposits, withdrawals, balance, history |
| **Backend** | All users' data (if they trust backend) |
| **Regulator** | Specific user's data (with court order + backend cooperation) |

---

## Code References

- **CommitmentData events**: `lib.rs:254-275`
- **Event parsing**: `anchor/tests/zkcash.ts:883-927`
- **Nullifier checking**: `lib.rs:329-364`
- **UTXO structure**: `anchor/tests/lib/utxo.ts:16-65`
- **Encryption utilities**: `anchor/tests/lib/utils.ts:22-100`

---

## Final Thoughts

Privacy Cash achieves a **clever balance**:
- ✅ On-chain commitments are unlinkable (privacy)
- ✅ `encrypted_output` allows discovery (usability)
- ✅ Events enable full transaction indexing (auditability)
- ⚠️ Backend cooperation enables compliance (trade-off)

**The encrypted_output mechanism is the key innovation** - it solves the "how do I know which UTXOs are mine?" problem without compromising on-chain privacy!
