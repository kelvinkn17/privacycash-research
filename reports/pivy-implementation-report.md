# PIVY Implementation Report: Practical, Private, Compliant

**Date**: October 18, 2025  
**Version**: Draft 1.0 (Aligned with Practical Architecture V2)

---

## Executive Summary

PIVY gives freelancers and businesses Venmo-level simplicity with self-custodied, privacy-preserving crypto payment links. Compared to Privacy Cash, PIVY keeps the same proven anonymity set mechanics but removes the centralized backend, cuts fees, and adds a cryptographically enforced compliance layer. Every deposit and withdrawal is logged on-chain in a tamper-proof stream that can be fully decoded by anyone holding the MetaView private key (the user, PIVY’s mobile backend, or—if voluntarily shared—regulators), while ordinary observers cannot correlate payment link deposits or withdrawals.

This report focuses on the real-world MVP we are actively building: single global pool, regulatory metadata backdoor, and receiver-centric privacy. It replaces the older ZK-heavy architecture with the simple, fast, auditable flow captured in `04_pivy_practical_architecture_v2.md`.

---

## Product Context: Payment Links With Receiver Privacy

- **User story**: Kelvin publishes `pivy.me/kelvin`. John, Angel, and other clients can each fund 1,000 USDC using their own wallets. Funds land inside the shared PIVY pool as shielded notes. Anyone monitoring the chain sees deposits into the pool but cannot link them to Kelvin without decrypting regulatory metadata.
- **Withdrawal**: Kelvin later withdraws 2,000 USDC using the MetaSpend key. Observers cannot tell that 2,000 USDC originated from those earlier deposits; they only see a shielded withdrawal. Regulators with a lawful order can request disclosure for specific commitments and reconstruct the provenance.
- **No centralized servers**: All protocol rules, including deposit/withdraw logging, live fully on-chain on Sui. The only “server” component is a stateless relayer (optional) plus the optional MetaView key backup service for mobile users; neither has unilateral control of funds.

---

## Core Technical Themes

- **Privacy Cash foundation**: We reuse the Groth16-style privacy circuit, Merkle tree commitments, and nullifier logic, ported to the Sui Move environment. This keeps performance and auditing predictable while leveraging Sui’s object-based state model.
- **Compliance metadata**: Each commitment now carries encrypted regulatory metadata (timestamp, amount range, deposit signature, depositor address, account pointer) stored on-chain alongside the encrypted note. The payload is encrypted with the recipient’s MetaView public key so only holders of the matching MetaView private key (user, trusted PIVY backend, or authorized regulator) can decrypt it.
- **Receiver-focused UX**: Wallets derive payment link deposit addresses using MetaView keys, keeping senders anonymous while letting the recipient aggregate funds under a single unseen pointer (`account_tag`).
- **Tamper-proof logging**: Every deposit and withdrawal emits append-only events and updates an internal Commitment Log Merkle Tree, making tampering or omission cryptographically infeasible.

---

## Meta Keypairs and Account Pointers

Each PIVY user generates two master keypairs that also power the compliance story:

1. **MetaSpend (public `ms_pub`, private `ms_priv`)**  
   - Controls shielded outputs.  
   - Signs withdrawals and reshielding transactions.  
   - Never shared; equivalent to Tornado/Tornado Cash private key.

2. **MetaView (public `mv_pub`, private `mv_priv`)**  
   - Used to derive payment link receiving codes and to compute internal account pointers.  
   - `mv_priv` allows the holder to scan the pool, recognize deposits, and decrypt related regulatory metadata. Whoever holds `mv_priv` (the user, trusted PIVY backend, or a regulator with delegated access) can reconstruct the full transaction history for that payment link.

### Account Tags (`pivy_account_tag`)

- We derive a deterministic pointer that binds deposits and withdrawals to a pseudonymous account without revealing the owner.  
  `account_tag = Poseidon(mv_pub || link_slug || pool_id)`  
  - `mv_pub` is fixed for the user; `link_slug` (e.g., `kelvin`) is registered when the payment link is created.  
  - Because both inputs are stable, the pointer cannot drift unless the user intentionally rotates either the MetaView key or the public link.  
  - Stored only inside encrypted regulatory metadata.  
  - Anyone who holds `mv_priv` can decrypt metadata and aggregate deposits/withdrawals per `account_tag` (e.g., “`pivy123` currently holds 0.5 SOL”).
- The same tag is embedded during withdrawals, allowing any MetaView private key holder to correlate exits to the same pseudonymous account.
- Without `mv_priv`, observers cannot match deposits and withdrawals or learn the tag contents.

---

## Deposit Flow with Immutable Logging

1. **Sender preparation**  
   - User visits `pivy.me/kelvin`, enters amount (e.g., 1,000 USDC).  
   - Frontend derives an ephemeral key for the payer and computes a stealth destination note using Kelvin’s `mv_pub`.  
   - Generates standard Privacy Cash proof for deposit. **Metaspend private key is never used or exposed during deposits**; only public data (`ms_pub`, `mv_pub`) is required on the payer side.

2. **Regulatory metadata assembly**  
   - Constructs `RegulatoryDepositMetadata` (only decryptable with `mv_priv`):
     ```typescript
     {
       version: 1,
       depositorAddress: payerPublicKey,
       depositTxSignature,
       timestamp: now(),
       amountBand: "100-1_000 USDC",   // Range only
       accountTag: Poseidon(mv_pub || epoch || pool_id),
       poolId: "mainnet-global",
       merkleIndex: projectedIndex,
       commitmentType: "deposit"
     }
     ```
   - Encrypts metadata with Kelvin’s `mv_pub`-derived viewing encryption key (no MetaSpend secrets ever leave Kelvin’s device).

3. **On-chain execution**  
- Sui Move smart contract verifies the ZK deposit proof, ensures Merkle root validity, and appends commitment.  
   - Emits `PIVYDepositLogged` event (index, note commitment hash, encrypted user ciphertext, encrypted regulatory metadata, commitment log Merkle root after append).  
   - Updates the Compliance Log Tree: `commit_log_root = Merkle(commit_log_root, keccak(index || commitment || metadata_hash))`. The tree stores only ciphertext references; plaintext stays accessible exclusively through `mv_priv`.

4. **Tamper resistance**  
- Because both the commitment and log tree roots are stored in the pool account and referenced by subsequent proofs, any attempt to modify past deposits invalidates the Merkle root history.  
- Solana’s ledger itself is append-only; combined with Merkle commitments, this makes deposit logs immutable.

5. **Result**  
   - Everyone sees an anonymous deposit into the pool.  
   - Anyone who holds `mv_priv` (Kelvin, PIVY backend, or voluntarily a regulator) can reconstruct the deposit timeline for `account_tag`, but without `mv_priv` observers learn nothing beyond pool-level activity.

### Example Scenario

- Wallet `0xABC` pays Kelvin’s link five times, each 0.1 SOL.  
- After five deposits, encrypted logs indicate `account_tag = pivy123` has an internal balance of 0.5 SOL.  
- Without decrypting metadata, outside observers cannot see that the same party funded the link multiple times.

---

## Withdrawal Flow with Pointer Continuity

1. **Kelvin scans pool** using `mv_priv`, finds notes destined to him, and selects 0.1 SOL UTXO to withdraw.

2. **Regulatory withdrawal metadata** mirrors deposit structure and remains decryptable only with `mv_priv`:
   ```typescript
   {
     version: 1,
     withdrawalTxSignature,
     timestamp: now(),
     amountBand: "0-0.5 SOL",
     nullifierHash,
     destinationAddressType: "external_wallet",
     accountTag: Poseidon(mv_pub || epoch || pool_id),
     complianceReason: "user_request",
     commitmentType: "withdraw"
   }
   ```

3. **Program verification**  
   - Checks nullifier non-reuse, validates ZK withdrawal proof, ensures log tree root continuity.  
   - Emits `PIVYWithdrawalLogged` with encrypted metadata and updates the compliance log Merkle tree.

4. **Pointer preservation**  
   - Because both deposit and withdrawal metadata reuse the same `account_tag`, any MetaView private key holder can prove “`pivy123` withdrew 0.1 SOL to address `xx...`.”

5. **Backdoor access flow**  
   - Court order (or equivalent) compels disclosure. User or PIVY compliance officer can hand over `mv_priv` (or derived viewing key) so investigators can decrypt the exact commitments requested.  
   - Operator generates a human-readable report (deposit timeline, withdrawal history for `account_tag`) and delivers it to the authority.  
   - The protocol only emits an optional `RegulatoryDisclosureEvent` if we decide to publicly acknowledge disclosures.

---

## Comparison vs. Privacy Cash

| Feature | Privacy Cash | PIVY | Impact |
|---------|--------------|------|--------|
| Backend dependency | Centralized REST API (`api3.privacycash.org`) | None – Anchor program only | Removes censorable bottleneck |
| Logging integrity | Server-side logs, mutable | On-chain events + Merkle log tree | Impossible to tamper without invalidating state |
| Compliance linking | Not available | Encrypted metadata with `account_tag` | Regulators get traceability, public doesn’t |
| Payment UX | Generic mixing | Payment-link optimized (MetaView stealth addresses) | Fits freelancers and creators |
| Withdrawal fee | 0.25% | 0.10-0.15% | 40-60% cheaper |
| Proof generation | ~3 s deposit | ~3.1 s (deposit) / ~3.3 s (withdraw) | Practical for consumer UX |

---

## Implementation Breakdown

### Smart Contract Additions

- **`PoolAccount`** includes `regulatory_pubkey`, `compliance_log_root`, `log_index`, and fee fields already detailed in Architecture V2.  
- **Events**: `PIVYDepositLogged`, `PIVYWithdrawalLogged`, `ComplianceLogUpdated`, `RegulatoryDisclosureEvent`.  
- **Instruction hooks**: Deposit/withdraw functions append to `ComplianceLog` data structure after verifying ZK proofs.

### Client SDK Changes

- `MetaKeyManager` handles MetaView/MetaSpend generation, stealth address derivation, and log scanning.  
- `ComplianceMetadataBuilder` constructs deposit/withdraw encryption payloads.  
- `AccountTagTracker` keeps local view of balances by decrypting metadata with `mv_priv` (useful for custodial clients or compliance reviews).

### Compliance Service (Optional)

   - Stateless service listens to on-chain events, stores ciphertexts, and responds to regulator-approved disclosure requests with decrypted transactions.  
   - Holds user-provided MetaView private keys in secure enclaves when customers opt in (e.g., PIVY mobile app backup).  
   - Provides API endpoints for regulators and enterprise customers to fetch audit trails once authorized.

---

## Security & Tamper Resistance

- **Append-only ledger**: Solana history + Merkle log root ensures deposit/withdraw events are immutable.  
- **Nullifier enforcement**: Prevents double spends; compliance metadata includes nullifier hash to tie decrypted records to actual withdrawals.  
- **MetaView key as the viewing backdoor**: Whoever holds `mv_priv` can decrypt all associated metadata; PIVY can escrow backups (HSM or secure enclave) for mobile users. Internal policy governs when that key is handed over.  
- **Self-custody resilience**: Even if PIVY’s databases or mobile backend fail, users who retain their MetaView/MetaSpend keys can reconstruct balances directly from on-chain data and withdraw via Sui wallets.
- **Optional transparency hooks**: If desired, we can emit public disclosure events or maintain a transparency log, but the base MVP does not require it.

---

## Compliance Reporting Walkthrough

1. **Request**: Subpoena cites specific commitment index or `account_tag` (if known from prior disclosure).  
2. **Authorization**: Compliance officer verifies the order meets policy and (optionally) logs an internal case ID.  
3. **Decryption**: Officer retrieves the corresponding MetaView private key (user-provided, escrowed, or newly shared) and decrypts the relevant metadata entries.  
4. **Report**: Generated report contains timeline, amount ranges, depositor addresses, withdrawal destinations, and running balance for `account_tag`.  
5. **Recordkeeping**: Findings stored in secured compliance archive; public transparency entry optional depending on policy.

---

## Performance & Scalability

- **Proof performance**: Keeping only the privacy proof plus light metadata encryption keeps UX within 3-3.5 seconds per operation on consumer hardware.  
- **On-chain cost**: Metadata payloads limited to a few hundred bytes; deposit and withdrawal accounts remain within Solana compute limits after recent Anchor optimizations.  
- **Log growth**: Compliance log tree uses 32-byte hashes; even with millions of operations per year, on-chain storage remains manageable. Off-chain indexers can archive full event data for analytics.

---

## Roadmap Alignment

| Phase | Focus | Key Deliverables |
|-------|-------|------------------|
| Month 1-2 (MVP) | Single pool, receiver privacy | Meta key wallet tooling, encrypted metadata, compliance log events |
| Month 3-4 (Compliance Ops) | Compliance operations | MetaView key custody policy, disclosure workflow dashboard, optional transparency log |
| Month 5-6 (Launch) | Mainnet readiness | External audit, legal opinions, fee calibration, marketing integrations |
| Post-launch | Enhanced analytics | Optional selective-disclosure proofs, enterprise reporting APIs |

---

## Key Takeaways

- PIVY matches Privacy Cash’s anonymity while removing centralized infrastructure and adding verifiable compliance.
- Deposit and withdrawal logs are immutable, cryptographically linked to pseudonymous account tags only regulators can decode, satisfying the “impossible to tamper” requirement.
- MetaView/MetaSpend key separation enables frictionless payment links, clean scanning for recipients, and delegated compliance reporting without exposing control keys.
- Faster, cheaper, and receiver-focused design makes PIVY the practical privacy link solution we set out to build.

---

**Next Steps**

1. Finalize MetaKey SDK module and document wallet integration.  
2. Implement compliance log Merkle tree and event emission in the Sui Move program.  
3. Finalize MetaView key custody/backup policy for mobile clients (no DAO ceremony needed).  
4. Draft regulator-facing summary referencing this report for upcoming outreach.


