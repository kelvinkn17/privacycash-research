use anchor_lang::prelude::*;
use light_hasher::Poseidon;
use anchor_lang::solana_program::sysvar::rent::Rent;

declare_id!("PivyP11111111111111111111111111111111111111");

pub mod merkle_tree;
pub mod utils;
pub mod groth16;
pub mod errors;
pub mod proof_gen;

#[cfg(test)]
pub mod tests;

use merkle_tree::MerkleTree;

// Constants
const MERKLE_TREE_HEIGHT: u8 = 26;
const MAX_BUCKET_SIZE: u8 = 100; // Maximum commitments per bucket

#[cfg(any(feature = "localnet", test))]
pub const ADMIN_PUBKEY: Option<Pubkey> = None;

#[cfg(not(any(feature = "localnet", test)))]
pub const ADMIN_PUBKEY: Option<Pubkey> = Some(pubkey!("AWexibGxNFKTa1b5R5MN4PJr9HWnWRwf8EW9g8cLx3dM"));

#[program]
pub mod pivy {
    use crate::utils::{verify_withdrawal_proof, VERIFYING_KEY};
    use super::*;

    /// Initialize the PIVY program with merkle tree and global config
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        if let Some(admin_key) = ADMIN_PUBKEY {
            require!(ctx.accounts.authority.key().eq(&admin_key), ErrorCode::Unauthorized);
        }

        let tree_account = &mut ctx.accounts.tree_account.load_init()?;
        tree_account.authority = ctx.accounts.authority.key();
        tree_account.next_index = 0;
        tree_account.root_index = 0;
        tree_account.bump = ctx.bumps.tree_account;
        tree_account.max_deposit_amount = 1_000_000_000_000; // 1000 SOL default limit
        tree_account.height = MERKLE_TREE_HEIGHT;
        tree_account.root_history_size = 100;

        MerkleTree::initialize::<Poseidon>(tree_account)?;

        let pool_account = &mut ctx.accounts.pool_account;
        pool_account.authority = ctx.accounts.authority.key();
        pool_account.bump = ctx.bumps.pool_account;

        let global_config = &mut ctx.accounts.global_config;
        global_config.authority = ctx.accounts.authority.key();
        global_config.withdrawal_fee_rate = 25; // 0.25% (25 basis points)
        global_config.bump = ctx.bumps.global_config;

        let max_deposit = tree_account.max_deposit_amount;
        let withdrawal_fee = global_config.withdrawal_fee_rate;
        msg!("PIVY initialized: height {}, max deposit {}, withdrawal fee {}bps",
            MERKLE_TREE_HEIGHT, max_deposit, withdrawal_fee);
        Ok(())
    }

    /// Deposit SOL to a PIVY account using recipient's meta public keys
    /// The client generates the commitment hash(amount, metaSpend_pub, blinding)
    /// and encrypts the output for the recipient using metaView_pub
    pub fn deposit(
        ctx: Context<Deposit>,
        commitment: [u8; 32],
        encrypted_output: Vec<u8>,
        amount: u64,
        blinded_account_id: [u8; 32],
    ) -> Result<()> {
        let tree_account = &mut ctx.accounts.tree_account.load_mut()?;
        let bucket_account = &mut ctx.accounts.bucket_account;

        // Check deposit limit
        require!(
            amount <= tree_account.max_deposit_amount,
            ErrorCode::DepositLimitExceeded
        );

        // Transfer SOL from depositor to pool
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.depositor.to_account_info(),
                    to: ctx.accounts.pool_account.to_account_info(),
                },
            ),
            amount,
        )?;

        // Add commitment to merkle tree
        let index = tree_account.next_index;
        MerkleTree::append::<Poseidon>(commitment, tree_account)?;

        // Add to bucket for aggregation
        bucket_account.add_commitment(commitment, amount)?;

        // Emit event with encrypted output for recipient
        emit!(DepositEvent {
            index,
            commitment,
            encrypted_output,
            blinded_account_id,
            amount,
        });

        msg!("Deposit successful: {} lamports, commitment added at index {}", amount, index);
        Ok(())
    }

    /// Withdraw from aggregated bucket
    /// User proves ownership of the entire bucket balance in ONE transaction
    /// using ZK proof that verifies knowledge of all commitment secrets
    pub fn withdraw(
        ctx: Context<Withdraw>,
        proof: WithdrawalProof,
        withdrawal_amount: u64,
    ) -> Result<()> {
        let tree_account = &mut ctx.accounts.tree_account.load_mut()?;
        let bucket_account = &mut ctx.accounts.bucket_account;
        let global_config = &ctx.accounts.global_config;

        // Check bucket hasn't been spent
        require!(!bucket_account.is_spent, ErrorCode::BucketAlreadySpent);

        // Verify bucket root is in merkle tree history
        require!(
            MerkleTree::is_known_root(&tree_account, proof.bucket_root),
            ErrorCode::UnknownRoot
        );

        // Calculate fee
        let fee = (withdrawal_amount as u128)
            .checked_mul(global_config.withdrawal_fee_rate as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::ArithmeticOverflow)? as u64;

        // Verify total amount matches bucket balance
        require!(
            withdrawal_amount <= bucket_account.total_balance,
            ErrorCode::InsufficientBalance
        );

        // Verify ZK proof
        require!(
            verify_withdrawal_proof(proof.clone(), VERIFYING_KEY),
            ErrorCode::InvalidProof
        );

        let pool_account_info = ctx.accounts.pool_account.to_account_info();
        let rent = Rent::get()?;
        let rent_exempt_minimum = rent.minimum_balance(pool_account_info.data_len());

        let total_required = withdrawal_amount
            .checked_add(fee)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_add(rent_exempt_minimum)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        require!(
            pool_account_info.lamports() >= total_required,
            ErrorCode::InsufficientFundsForWithdrawal
        );

        // Transfer withdrawal amount to recipient
        let recipient_account_info = ctx.accounts.recipient.to_account_info();
        let pool_balance = pool_account_info.lamports();
        let recipient_balance = recipient_account_info.lamports();

        **pool_account_info.try_borrow_mut_lamports()? = pool_balance
            .checked_sub(withdrawal_amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        **recipient_account_info.try_borrow_mut_lamports()? = recipient_balance
            .checked_add(withdrawal_amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Transfer fee if applicable
        if fee > 0 {
            let fee_recipient_info = ctx.accounts.fee_recipient.to_account_info();
            let pool_balance = pool_account_info.lamports();
            let fee_recipient_balance = fee_recipient_info.lamports();

            **pool_account_info.try_borrow_mut_lamports()? = pool_balance
                .checked_sub(fee)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            **fee_recipient_info.try_borrow_mut_lamports()? = fee_recipient_balance
                .checked_add(fee)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }

        // Mark bucket as spent to prevent double-spending
        bucket_account.is_spent = true;
        bucket_account.total_balance = bucket_account.total_balance
            .checked_sub(withdrawal_amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        emit!(WithdrawalEvent {
            bucket_id: ctx.accounts.bucket_account.key(),
            amount: withdrawal_amount,
            fee,
            recipient: ctx.accounts.recipient.key(),
        });

        msg!("Withdrawal successful: {} lamports withdrawn, fee {} lamports", withdrawal_amount, fee);
        Ok(())
    }

    /// Update the maximum deposit amount limit
    pub fn update_deposit_limit(ctx: Context<UpdateDepositLimit>, new_limit: u64) -> Result<()> {
        let tree_account = &mut ctx.accounts.tree_account.load_mut()?;
        tree_account.max_deposit_amount = new_limit;
        msg!("Deposit limit updated to: {} lamports", new_limit);
        Ok(())
    }

    /// Update global configuration
    pub fn update_global_config(
        ctx: Context<UpdateGlobalConfig>,
        withdrawal_fee_rate: Option<u16>,
    ) -> Result<()> {
        let global_config = &mut ctx.accounts.global_config;

        if let Some(rate) = withdrawal_fee_rate {
            require!(rate <= 10000, ErrorCode::InvalidFeeRate);
            global_config.withdrawal_fee_rate = rate;
            msg!("Withdrawal fee rate updated to: {} basis points", rate);
        }

        Ok(())
    }
}

// ============================================================================
// Events
// ============================================================================

#[event]
pub struct DepositEvent {
    pub index: u64,
    pub commitment: [u8; 32],
    pub encrypted_output: Vec<u8>,
    pub blinded_account_id: [u8; 32],
    pub amount: u64,
}

#[event]
pub struct WithdrawalEvent {
    pub bucket_id: Pubkey,
    pub amount: u64,
    pub fee: u64,
    pub recipient: Pubkey,
}

// ============================================================================
// Proof Structures
// ============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct WithdrawalProof {
    pub proof_a: [u8; 64],
    pub proof_b: [u8; 128],
    pub proof_c: [u8; 64],
    pub bucket_root: [u8; 32],
    pub nullifier: [u8; 32],
    pub meta_spend_pubkey: [u8; 32],
}

// ============================================================================
// Account Contexts
// ============================================================================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 8 + (32 * MERKLE_TREE_HEIGHT as usize) + 32 + (32 * 100) + 8 + 8 + 1 + 1 + 1 + 5,
        seeds = [b"merkle_tree"],
        bump
    )]
    pub tree_account: AccountLoader<'info, MerkleTreeAccount>,

    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 1,
        seeds = [b"pool"],
        bump
    )]
    pub pool_account: Account<'info, PoolAccount>,

    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 2 + 1,
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(commitment: [u8; 32], encrypted_output: Vec<u8>, amount: u64, blinded_account_id: [u8; 32])]
pub struct Deposit<'info> {
    #[account(
        mut,
        seeds = [b"merkle_tree"],
        bump = tree_account.load()?.bump
    )]
    pub tree_account: AccountLoader<'info, MerkleTreeAccount>,

    #[account(
        init_if_needed,
        payer = depositor,
        space = 8 + 1 + 8 + 1 + (32 * MAX_BUCKET_SIZE as usize) + 1,
        seeds = [b"bucket", blinded_account_id.as_ref()],
        bump
    )]
    pub bucket_account: Account<'info, BucketAccount>,

    #[account(
        mut,
        seeds = [b"pool"],
        bump = pool_account.bump
    )]
    pub pool_account: Account<'info, PoolAccount>,

    #[account(mut)]
    pub depositor: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(proof: WithdrawalProof, withdrawal_amount: u64)]
pub struct Withdraw<'info> {
    #[account(
        mut,
        seeds = [b"merkle_tree"],
        bump = tree_account.load()?.bump
    )]
    pub tree_account: AccountLoader<'info, MerkleTreeAccount>,

    #[account(
        mut,
        seeds = [b"bucket", proof.nullifier.as_ref()],
        bump
    )]
    pub bucket_account: Account<'info, BucketAccount>,

    #[account(
        mut,
        seeds = [b"pool"],
        bump = pool_account.bump
    )]
    pub pool_account: Account<'info, PoolAccount>,

    #[account(
        seeds = [b"global_config"],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    /// CHECK: Recipient can be any account
    pub recipient: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Fee recipient can be any account
    pub fee_recipient: UncheckedAccount<'info>,

    pub signer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateDepositLimit<'info> {
    #[account(
        mut,
        seeds = [b"merkle_tree"],
        bump = tree_account.load()?.bump,
        has_one = authority @ ErrorCode::Unauthorized
    )]
    pub tree_account: AccountLoader<'info, MerkleTreeAccount>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateGlobalConfig<'info> {
    #[account(
        mut,
        seeds = [b"global_config"],
        bump = global_config.bump,
        has_one = authority @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,
}

// ============================================================================
// Account Structures
// ============================================================================

#[account]
pub struct PoolAccount {
    pub authority: Pubkey,
    pub bump: u8,
}

#[account]
pub struct GlobalConfig {
    pub authority: Pubkey,
    pub withdrawal_fee_rate: u16, // basis points (0-10000)
    pub bump: u8,
}

/// Bucket account aggregates multiple deposits for a single PIVY account
/// Uses Pedersen commitment homomorphic property: C1 + C2 + ... = C_total
#[account]
pub struct BucketAccount {
    pub commitment_count: u8,
    pub total_balance: u64,
    pub is_spent: bool,
    pub commitments: [[u8; 32]; MAX_BUCKET_SIZE as usize],
    pub bump: u8,
}

impl Default for BucketAccount {
    fn default() -> Self {
        Self {
            commitment_count: 0,
            total_balance: 0,
            is_spent: false,
            commitments: [[0u8; 32]; MAX_BUCKET_SIZE as usize],
            bump: 0,
        }
    }
}

impl BucketAccount {
    pub fn add_commitment(&mut self, commitment: [u8; 32], amount: u64) -> Result<()> {
        require!(
            self.commitment_count < MAX_BUCKET_SIZE,
            ErrorCode::BucketFull
        );

        self.commitments[self.commitment_count as usize] = commitment;
        self.commitment_count += 1;
        self.total_balance = self.total_balance
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        Ok(())
    }
}

#[account(zero_copy(unsafe))]
pub struct MerkleTreeAccount {
    pub authority: Pubkey,
    pub next_index: u64,
    pub subtrees: [[u8; 32]; MERKLE_TREE_HEIGHT as usize],
    pub root: [u8; 32],
    pub root_history: [[u8; 32]; 100],
    pub root_index: u64,
    pub max_deposit_amount: u64,
    pub height: u8,
    pub root_history_size: u8,
    pub bump: u8,
    pub _padding: [u8; 5],
}

// ============================================================================
// Errors
// ============================================================================

#[error_code]
pub enum ErrorCode {
    #[msg("Not authorized to perform this action")]
    Unauthorized,
    #[msg("Root is not known in the tree")]
    UnknownRoot,
    #[msg("Insufficient balance in bucket")]
    InsufficientBalance,
    #[msg("Insufficient funds for withdrawal")]
    InsufficientFundsForWithdrawal,
    #[msg("Proof is invalid")]
    InvalidProof,
    #[msg("Arithmetic overflow/underflow occurred")]
    ArithmeticOverflow,
    #[msg("Deposit limit exceeded")]
    DepositLimitExceeded,
    #[msg("Invalid fee rate: must be between 0 and 10000 basis points")]
    InvalidFeeRate,
    #[msg("Merkle tree is full: cannot add more leaves")]
    MerkleTreeFull,
    #[msg("Bucket is already spent")]
    BucketAlreadySpent,
    #[msg("Bucket is full: maximum commitments reached")]
    BucketFull,
}
