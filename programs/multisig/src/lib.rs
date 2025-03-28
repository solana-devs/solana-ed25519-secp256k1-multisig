use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_lang::solana_program::instruction::Instruction;
use solana_program::sysvar::instructions::{load_instruction_at_checked, ID as IX_ID};
use std::convert::Into;
use std::ops::Deref;

pub mod utils;

declare_id!("CQGjzX8tN5AdEjzRkRJoVsxQtbdYpwvFJ8nVm6SQDJJp");

#[program]
pub mod multisig {
    use super::*;

    // Initializes a new multisig account with a set of owners and a threshold.
    pub fn create_multisig(
        ctx: Context<CreateMultisig>,
        owners: Vec<[u8; 20]>,
        threshold: u64,
        nonce: u8,
    ) -> Result<()> {
        assert_unique_owners(&owners)?;
        require!(
            threshold > 0 && threshold <= owners.len() as u64,
            ErrorCodeMultiSig::InvalidThreshold
        );
        require!(!owners.is_empty(), ErrorCodeMultiSig::InvalidOwnersLen);

        let multisig = &mut ctx.accounts.multisig;
        multisig.owners = owners;
        multisig.threshold = threshold;
        multisig.nonce = nonce;
        multisig.owner_set_seqno = 0;
        multisig.last_transaction_id = 0;
        Ok(())
    }

    // Creates a new transaction account, automatically signed by the creator,
    // which must be one of the owners of the multisig.
    pub fn create_transaction(
        ctx: Context<CreateTransaction>,
        pid: Pubkey,
        accs: Vec<TransactionAccount>,
        data: Vec<u8>,
        eth_address: [u8; 20],
        sig: [u8; 64],
        recovery_id: u8,
    ) -> Result<()> {
        // Get what should be the Secp256k1Program instruction
        let ix: Instruction = load_instruction_at_checked(0, &ctx.accounts.ix_sysvar)?;

        let last_transaction_id = ctx.accounts.multisig.last_transaction_id;
        let mut bytes_last_transaction_id = last_transaction_id.to_le_bytes().to_vec();
        bytes_last_transaction_id.push(ctx.accounts.multisig.nonce);
        let msg = bytes_last_transaction_id;
        // Check that ix is what we expect to have been sent
        utils::verify_secp256k1_ix(&ix, &eth_address, &msg, &sig, recovery_id)?;

        let owner_index = ctx
            .accounts
            .multisig
            .owners
            .iter()
            .position(|a| a == &eth_address)
            .ok_or(ErrorCodeMultiSig::InvalidOwner)?;

        let mut signers = Vec::new();
        signers.resize(ctx.accounts.multisig.owners.len(), false);
        signers[owner_index] = true;

        let tx = &mut ctx.accounts.transaction;
        tx.program_id = pid;
        tx.accounts = accs;
        tx.data = data;
        tx.signers = signers;
        tx.multisig = ctx.accounts.multisig.key();
        tx.did_execute = false;
        tx.owner_set_seqno = ctx.accounts.multisig.owner_set_seqno;
        tx.transaction_id = last_transaction_id;

        ctx.accounts.multisig.last_transaction_id += 1;

        Ok(())
    }

    // Approves a transaction on behalf of an owner of the multisig.
    pub fn approve(
        ctx: Context<Approve>,
        eth_address: [u8; 20],
        sig: [u8; 64],
        recovery_id: u8,
    ) -> Result<()> {
        // Get what should be the Secp256k1Program instruction
        let ix: Instruction = load_instruction_at_checked(0, &ctx.accounts.ix_sysvar)?;

        let last_transaction_id = ctx.accounts.multisig.last_transaction_id;
        let mut bytes_last_transaction_id = last_transaction_id.to_le_bytes().to_vec();
        bytes_last_transaction_id.push(ctx.accounts.multisig.nonce);
        bytes_last_transaction_id.push(1); //just to make it different from create_transaction msg
        let msg = bytes_last_transaction_id;

        // Check that ix is what we expect to have been sent
        utils::verify_secp256k1_ix(&ix, &eth_address, &msg, &sig, recovery_id)?;
        let owner_index = ctx
            .accounts
            .multisig
            .owners
            .iter()
            .position(|a| a == &eth_address)
            .ok_or(ErrorCodeMultiSig::InvalidOwner)?;

        ctx.accounts.transaction.signers[owner_index] = true;

        Ok(())
    }

    // Set owners and threshold at once.
    pub fn update_owners<'info>(
        ctx: Context<'_, '_, '_, 'info, Auth<'info>>,
        owners: Vec<[u8; 20]>,
    ) -> Result<()> {
        // Sets the owners field on the multisig. The only way this can be invoked
        // is via a recursive call from execute_transaction -> set_owners.
        assert_unique_owners(&owners)?;
        require!(!owners.is_empty(), ErrorCodeMultiSig::InvalidOwnersLen);

        let multisig = &mut ctx.accounts.multisig;

        if (owners.len() as u64) < multisig.threshold {
            multisig.threshold = owners.len() as u64;
        }

        multisig.owners = owners;
        multisig.owner_set_seqno += 1;
        Ok(())
    }

    // Set owners and threshold at once.
    pub fn update_threshold<'info>(
        ctx: Context<'_, '_, '_, 'info, Auth<'info>>,
        threshold: u64,
    ) -> Result<()> {
        // Changes the execution threshold of the multisig. The only way this can be
        // invoked is via a recursive call from execute_transaction ->
        // change_threshold.
        require!(threshold > 0, ErrorCodeMultiSig::InvalidThreshold);
        if threshold > ctx.accounts.multisig.owners.len() as u64 {
            return Err(ErrorCodeMultiSig::InvalidThreshold.into());
        }
        let multisig = &mut ctx.accounts.multisig;
        multisig.threshold = threshold;
        Ok(())
    }

    // Executes the given transaction if threshold owners have signed it.
    pub fn execute_transaction(ctx: Context<ExecuteTransaction>) -> Result<()> {
        // Has this been executed already?
        if ctx.accounts.transaction.did_execute {
            return Err(ErrorCodeMultiSig::AlreadyExecuted.into());
        }

        // Do we have enough signers.
        let sig_count = ctx
            .accounts
            .transaction
            .signers
            .iter()
            .filter(|&did_sign| *did_sign)
            .count() as u64;
        if sig_count < ctx.accounts.multisig.threshold {
            return Err(ErrorCodeMultiSig::NotEnoughSigners.into());
        }

        // Execute the transaction signed by the multisig.
        let mut ix: Instruction = (*ctx.accounts.transaction).deref().into();
        ix.accounts = ix
            .accounts
            .iter()
            .map(|acc| {
                let mut acc = acc.clone();
                if &acc.pubkey == ctx.accounts.multisig_signer.key {
                    acc.is_signer = true;
                }
                acc
            })
            .collect();
        let multisig_key = ctx.accounts.multisig.key();
        let seeds = &[multisig_key.as_ref(), &[ctx.accounts.multisig.nonce]];
        let signer = &[&seeds[..]];
        let accounts = ctx.remaining_accounts;
        solana_program::program::invoke_signed(&ix, accounts, signer)?;

        // Burn the transaction to ensure one time use.
        ctx.accounts.transaction.did_execute = true;

        Ok(())
    }
}

fn assert_unique_owners(owners: &[[u8; 20]]) -> Result<()> {
    for (i, owner) in owners.iter().enumerate() {
        require!(
            !owners.iter().skip(i + 1).any(|item| item == owner),
            ErrorCodeMultiSig::UniqueOwners
        )
    }
    Ok(())
}

#[derive(Accounts)]
pub struct CreateMultisig<'info> {
    #[account(zero, signer)]
    multisig: Box<Account<'info, Multisig>>,
}

#[derive(Accounts)]
pub struct CreateTransaction<'info> {
    pub multisig: Box<Account<'info, Multisig>>,
    #[account(zero, signer)]
    transaction: Box<Account<'info, Transaction>>,
    // One of the owners. Checked in the handler.
    proposer: Signer<'info>,

    /// CHECK: The address check is needed because otherwise
    /// the supplied Sysvar could be anything else.
    /// The Instruction Sysvar has not been implemented
    /// in the Anchor framework yet, so this is the safe approach.
    #[account(address = IX_ID)]
    pub ix_sysvar: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Approve<'info> {
    #[account(constraint = multisig.owner_set_seqno == transaction.owner_set_seqno)]
    multisig: Box<Account<'info, Multisig>>,
    #[account(mut, has_one = multisig)]
    transaction: Box<Account<'info, Transaction>>,
    // One of the multisig owners. Checked in the handler.
    owner: Signer<'info>,

    /// CHECK: The address check is needed because otherwise
    /// the supplied Sysvar could be anything else.
    /// The Instruction Sysvar has not been implemented
    /// in the Anchor framework yet, so this is the safe approach.
    #[account(address = IX_ID)]
    pub ix_sysvar: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Auth<'info> {
    #[account(mut)]
    multisig: Box<Account<'info, Multisig>>,
    #[account(
        seeds = [multisig.key().as_ref()],
        bump = multisig.nonce,
    )]
    multisig_signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecuteTransaction<'info> {
    #[account(constraint = multisig.owner_set_seqno == transaction.owner_set_seqno)]
    multisig: Box<Account<'info, Multisig>>,
    /// CHECK: multisig_signer is a PDA program signer. Data is never read or written to
    #[account(
        seeds = [multisig.key().as_ref()],
        bump = multisig.nonce,
    )]
    multisig_signer: UncheckedAccount<'info>,
    #[account(mut, has_one = multisig)]
    transaction: Box<Account<'info, Transaction>>,
}

#[account]
pub struct Multisig {
    pub owners: Vec<[u8; 20]>,
    pub threshold: u64,
    pub nonce: u8,
    pub owner_set_seqno: u32,
    pub last_transaction_id: u32,
}

#[account]
pub struct Transaction {
    // The multisig account this transaction belongs to.
    pub multisig: Pubkey,
    // Target program to execute against.
    pub program_id: Pubkey,
    // Accounts requried for the transaction.
    pub accounts: Vec<TransactionAccount>,
    // Instruction data for the transaction.
    pub data: Vec<u8>,
    // signers[index] is true iff multisig.owners[index] signed the transaction.
    pub signers: Vec<bool>,
    // Boolean ensuring one time execution.
    pub did_execute: bool,
    // Owner set sequence number.
    pub owner_set_seqno: u32,

    pub transaction_id: u32,
}

impl From<&Transaction> for Instruction {
    fn from(tx: &Transaction) -> Instruction {
        Instruction {
            program_id: tx.program_id,
            accounts: tx.accounts.iter().map(Into::into).collect(),
            data: tx.data.clone(),
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TransactionAccount {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl From<&TransactionAccount> for AccountMeta {
    fn from(account: &TransactionAccount) -> AccountMeta {
        match account.is_writable {
            false => AccountMeta::new_readonly(account.pubkey, account.is_signer),
            true => AccountMeta::new(account.pubkey, account.is_signer),
        }
    }
}

impl From<&AccountMeta> for TransactionAccount {
    fn from(account_meta: &AccountMeta) -> TransactionAccount {
        TransactionAccount {
            pubkey: account_meta.pubkey,
            is_signer: account_meta.is_signer,
            is_writable: account_meta.is_writable,
        }
    }
}

#[error_code]
pub enum ErrorCodeMultiSig {
    #[msg("The given owner is not part of this multisig.")]
    InvalidOwner,
    #[msg("Owners length must be non zero.")]
    InvalidOwnersLen,
    #[msg("Not enough owners signed this transaction.")]
    NotEnoughSigners,
    #[msg("Cannot delete a transaction that has been signed by an owner.")]
    TransactionAlreadySigned,
    #[msg("Overflow when adding.")]
    Overflow,
    #[msg("Cannot delete a transaction the owner did not create.")]
    UnableToDelete,
    #[msg("The given transaction has already been executed.")]
    AlreadyExecuted,
    #[msg("Threshold must be less than or equal to the number of owners.")]
    InvalidThreshold,
    #[msg("Owners must be unique")]
    UniqueOwners,
    #[msg("Signature verification failed.")]
    SigVerificationFailed,
}
