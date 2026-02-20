use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{Discriminator, InstructionData};
use solana_gpt_oracle::{ContextAccount, Identity};
use tuktuk_program::{
    compile_transaction,
    tuktuk::{
        cpi::{accounts::QueueTaskV0, queue_task_v0},
        program::Tuktuk,
        types::TriggerV0,
    },
    types::QueueTaskArgsV0,
    TransactionSourceV0,
};

declare_id!("DqVX6kVd1rZu3EA8NMQtpm33AARfB5Hm15YLFWJCSPJS");

#[program]
pub mod simple_agent {

    use super::*;

    const AGENT_DESC: &str = "you are a futuristic agent. you'll normally reply to user prompt.";

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.agent.set_inner(Agent {
            context: ctx.accounts.llm_context.key(),
        });

        let cpi_accounts = solana_gpt_oracle::cpi::accounts::CreateLlmContext {
            payer: ctx.accounts.payer.to_account_info(),
            context_account: ctx.accounts.llm_context.to_account_info(),
            counter: ctx.accounts.counter.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(ctx.accounts.oracle_program.to_account_info(), cpi_accounts);

        solana_gpt_oracle::cpi::create_llm_context(cpi_ctx, AGENT_DESC.to_string())?;

        Ok(())
    }

    pub fn interact_agent(ctx: Context<InteractAgent>, text: String) -> Result<()> {
        let cpi_program = ctx.accounts.oracle_program.to_account_info();
        let cpi_accounts = solana_gpt_oracle::cpi::accounts::InteractWithLlm {
            payer: ctx.accounts.payer.to_account_info(),
            interaction: ctx.accounts.interaction.to_account_info(),
            context_account: ctx.accounts.context_account.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        let disc: [u8; 8] = instruction::CallbackFromAgent::DISCRIMINATOR
            .try_into()
            .expect("Discriminator must be 8 bytes");

        solana_gpt_oracle::cpi::interact_with_llm(cpi_ctx, text, ID, disc, None)?;

        Ok(())
    }

    pub fn callback_from_agent(ctx: Context<CallbackFromAgent>, response: String) -> Result<()> {
        if !ctx.accounts.identity.to_account_info().is_signer {
            return Err(ProgramError::InvalidAccountData.into());
        }

        msg!("Agent Response: {:?}", response);
        Ok(())
    }

    pub fn close(_ctx: Context<CloseAgent>) -> Result<()> {
        Ok(())
    }

    pub fn schedule(ctx: Context<Schedule>, prompt: String, task_id: u16) -> Result<()> {
        let (compiled_tx, _) = compile_transaction(
            vec![Instruction {
                program_id: crate::ID,
                accounts: crate::__client_accounts_interact_agent::InteractAgent {
                    payer: ctx.accounts.payer.key(),
                    interaction: ctx.accounts.interaction.key(),
                    agent: ctx.accounts.agent.key(),
                    context_account: ctx.accounts.context_account.key(),
                    oracle_program: ctx.accounts.oracle_program.key(),
                    system_program: ctx.accounts.system_program.key(),
                }
                .to_account_metas(None)
                .to_vec(),

                data: crate::instruction::InteractAgent { text: prompt }.data(),
            }],
            vec![],
        )
        .unwrap();

        queue_task_v0(
            CpiContext::new_with_signer(
                ctx.accounts.tuktuk_program.to_account_info(),
                QueueTaskV0 {
                    payer: ctx.accounts.payer.to_account_info(),
                    queue_authority: ctx.accounts.queue_authority.to_account_info(),
                    task_queue: ctx.accounts.task_queue.to_account_info(),
                    task_queue_authority: ctx.accounts.task_queue_authority.to_account_info(),
                    task: ctx.accounts.task.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                },
                &[&["queue_authority".as_bytes(), &[ctx.bumps.queue_authority]]],
            ),
            QueueTaskArgsV0 {
                trigger: TriggerV0::Now,
                transaction: TransactionSourceV0::CompiledV0(compiled_tx),
                crank_reward: Some(1000001),
                free_tasks: 1,
                id: task_id,
                description: "agent_called".to_string(),
            },
        )
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = 8 + 32,
        seeds = [b"agent"],
        bump
    )]
    pub agent: Account<'info, Agent>,

    /// CHECK: Checked in oracle program
    #[account(mut)]
    pub llm_context: AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: initialized in cpi
    pub identity: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: initialized in cpi
    pub counter: UncheckedAccount<'info>,

    /// CHECK: Checked oracle id
    // #[account(address = solana_gpt_oracle::ID)]
    pub oracle_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(text: String)]
pub struct InteractAgent<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Checked in oracle program
    #[account(mut)]
    pub interaction: AccountInfo<'info>,
    #[account(seeds = [b"agent"], bump)]
    pub agent: Account<'info, Agent>,
    #[account(address = agent.context)]
    pub context_account: Account<'info, ContextAccount>,
    /// CHECK: Checked oracle id
    #[account(address = solana_gpt_oracle::ID)]
    pub oracle_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CallbackFromAgent<'info> {
    /// CHECK: Checked in oracle program
    pub identity: Account<'info, Identity>,
}

#[derive(Accounts)]
pub struct Schedule<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Checked in oracle program
    #[account(mut)]
    pub interaction: AccountInfo<'info>,
    #[account(seeds = [b"agent"], bump)]
    pub agent: Account<'info, Agent>,
    #[account(address = agent.context)]
    pub context_account: Account<'info, ContextAccount>,
    /// CHECK: Checked oracle id
    #[account(address = solana_gpt_oracle::ID)]
    pub oracle_program: AccountInfo<'info>,

    // TUKTUK
    #[account(mut)]
    /// CHECK: Don't need to parse this account, just using it in CPI
    pub task_queue: UncheckedAccount<'info>,
    /// CHECK: Don't need to parse this account, just using it in CPI
    pub task_queue_authority: UncheckedAccount<'info>,
    /// CHECK: Initialized in CPI
    #[account(mut)]
    pub task: UncheckedAccount<'info>,
    /// CHECK: Via seeds
    #[account(
        mut,
        seeds = [b"queue_authority"],
        bump
    )]
    pub queue_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub tuktuk_program: Program<'info, Tuktuk>,
}

#[derive(Accounts)]
pub struct CloseAgent<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        close = payer,
        seeds = [b"agent"],
        bump
    )]
    pub agent: Account<'info, Agent>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Agent {
    pub context: Pubkey,
}
