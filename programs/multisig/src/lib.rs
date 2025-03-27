use anchor_lang::prelude::*;

declare_id!("CQGjzX8tN5AdEjzRkRJoVsxQtbdYpwvFJ8nVm6SQDJJp");

#[program]
pub mod multisig {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
