use anchor_lang::prelude::*;
use arcium_anchor::{
    comp_def_offset, derive_cluster_pda, derive_comp_def_pda, derive_comp_pda, derive_execpool_pda,
    derive_mempool_pda, derive_mxe_pda, init_comp_def, queue_computation, ComputationOutputs,
    ARCIUM_CLOCK_ACCOUNT_ADDRESS, ARCIUM_STAKING_POOL_ACCOUNT_ADDRESS, CLUSTER_PDA_SEED,
    COMP_DEF_PDA_SEED, COMP_PDA_SEED, EXECPOOL_PDA_SEED, MEMPOOL_PDA_SEED, MXE_PDA_SEED,
};
use arcium_client::idl::arcium::{
    accounts::{
        ClockAccount, Cluster, ComputationDefinitionAccount, PersistentMXEAccount,
        StakingPoolAccount,
    },
    program::Arcium,
    types::{Argument, CallbackAccount},
    ID_CONST as ARCIUM_PROG_ID,
};
use arcium_macros::{
    arcium_callback, arcium_program, callback_accounts, init_computation_definition_accounts,
    queue_computation_accounts,
};

const COMP_DEF_OFFSET_INIT_PLAYER_SHIPS: u32 = comp_def_offset("init_player_ship_fleet_location");
const COMP_DEF_OFFSET_PLACE_SHIPS: u32 = comp_def_offset("place_ships");
const COMP_DEF_OFFSET_TAKE_TURN: u32 = comp_def_offset("take_turn");

declare_id!("HVaMfas33TSAihSxJUvDTpLPnXzHsW4WcD67FKAUDHQ2");

#[arcium_program]
pub mod confidential_battleship_game {
    use super::*;

    pub fn init_new_game_comp_def(ctx: Context<InitNewGameCompDef>) -> Result<()> {
        init_comp_def(ctx.accounts, true, None, None)?;
        Ok(())
    }

    pub fn new_game(
        ctx: Context<NewGame>,
        player_2_pubkey: Pubkey,
        player_1_arcium_pubkey: [u8; 32],
        player_2_arcium_pubkey: [u8; 32],
        computation_offset: u64,
        mxe_nonce: u128, // Nonce for the MXE to create the initial state.
    ) -> Result<()> {
        let game_account = &mut ctx.accounts.game_account;
        game_account.player_1 = ctx.accounts.payer.key();
        game_account.player_2 = player_2_pubkey;
        game_account.player_1_arcium_pubkey = player_1_arcium_pubkey;
        game_account.player_2_arcium_pubkey = player_2_arcium_pubkey;
        game_account.game_state = GameState::PlacingShips;
        game_account.player_1_ships_left = 17;
        game_account.player_2_ships_left = 17;
        game_account.bump = ctx.bumps.game_account;

        // Queue the computation to initialize the empty, encrypted fleet state.
        let args = vec![Argument::PlaintextU128(mxe_nonce)];

        queue_computation(
            ctx.accounts,
            computation_offset,
            args,
            vec![CallbackAccount {
                pubkey: ctx.accounts.game_account.key(),
                is_writable: true,
            }],
            None,
        )?;
        Ok(())
    }

    #[arcium_callback(encrypted_ix = "init_player_ship_fleet_location")]
    pub fn init_player_ship_fleet_location_callback(
        ctx: Context<InitPlayerShipFleetLocCallback>,
        output: ComputationOutputs,
    ) -> Result<()> {
        let bytes = if let ComputationOutputs::Bytes(bytes) = output {
            bytes
        } else {
            return Err(BattleShipErrorCode::AbortedComputation.into());
        };

        let fleet_state_nonce: [u8; 16] = bytes[0..16].try_into().unwrap();

        let encrypted_fleet_state: [[u8; 32]; 34] = bytes[16..]
            .chunks_exact(32)
            .map(|c| c.try_into().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let game_account = &mut ctx.accounts.game_account;
        game_account.fleet_state_nonce = fleet_state_nonce;
        game_account.encrypted_fleet_state = encrypted_fleet_state;

        emit!(GameCreated {
            game_pda: game_account.key(),
            player_1: game_account.player_1,
            player_2: game_account.player_2,
        });

        Ok(())
    }

    pub fn init_place_ships_comp_def(ctx: Context<InitPlaceShipsCompDef>) -> Result<()> {
        init_comp_def(ctx.accounts, true, None, None)?;
        Ok(())
    }

    pub fn place_ships_ix(
        ctx: Context<PlaceShipsIx>,
        computation_offset: u64,
        input_nonce: u128, // Nonce for the player's encrypted input.
        encrypted_ship_locations: [u8; 32], // Client-encrypted `PlaceShipsInputs`.
    ) -> Result<()> {
        let game_account = &ctx.accounts.game_account;
        require!(
            game_account.game_state == GameState::PlacingShips,
            BattleShipErrorCode::InvalidGameState
        );

        let payer_key = ctx.accounts.payer.key();
        let (is_player_1, player_arcium_pubkey) = if payer_key == game_account.player_1 {
            (true, game_account.player_1_arcium_pubkey)
        } else if payer_key == game_account.player_2 {
            (false, game_account.player_2_arcium_pubkey)
        } else {
            return Err(BattleShipErrorCode::UnauthorizedPlayer.into());
        };

        // arguments for the `place_ships` encrypted instruction.
        let args = vec![
            // 1. Arguments for `Enc<Shared, PlaceShipsInputs>`
            Argument::ArcisPubkey(player_arcium_pubkey),
            Argument::PlaintextU128(input_nonce),
            Argument::EncryptedU8(encrypted_ship_locations),
            // 2. Arguments for `Enc<Mxe, PlayerShipFleet>`
            Argument::PlaintextU128(u128::from_le_bytes(game_account.fleet_state_nonce)),
            Argument::Account(game_account.key(), 8 + 148, 1088), // key, data offset, data size
        ];

        queue_computation(
            ctx.accounts,
            computation_offset,
            args,
            vec![CallbackAccount {
                pubkey: game_account.key(),
                is_writable: true,
            }],
            None,
        )?;

        Ok(())
    }

    #[arcium_callback(encrypted_ix = "place_ships")]
    pub fn place_ships_callback(
        ctx: Context<PlaceShipsIxCallback>,
        output: ComputationOutputs,
    ) -> Result<()> {
        let bytes = if let ComputationOutputs::Bytes(bytes) = output {
            bytes
        } else {
            return Err(BattleShipErrorCode::AbortedComputation.into());
        };

        let new_fleet_state_nonce: [u8; 16] = bytes[0..16].try_into().unwrap();

        let new_encrypted_fleet_state: [[u8; 32]; 34] = bytes[16..]
            .chunks_exact(32)
            .map(|c| c.try_into().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let game_account = &mut ctx.accounts.game_account;
        game_account.fleet_state_nonce = new_fleet_state_nonce;
        game_account.encrypted_fleet_state = new_encrypted_fleet_state;

        emit!(ShipsPlaced {
            game_pda: game_account.key()
        });
        Ok(())
    }

    pub fn init_take_turn_comp_def(ctx: Context<InitTakeTurnCompDef>) -> Result<()> {
        init_comp_def(ctx.accounts, true, None, None)?;
        Ok(())
    }

    pub fn take_turn_ix(
        ctx: Context<TakeTurnIx>,
        computation_offset: u64,
        input_nonce: u128,         // Nonce for the player's encrypted guess.
        encrypted_guess: [u8; 32], // Client-encrypted `TakeTurnInputs`.
    ) -> Result<()> {
        let game_account = &mut ctx.accounts.game_account;
        let payer_key = ctx.accounts.payer.key();
        let game_account_key = game_account.key();

        let (is_player_1, player_arcium_pubkey) = match game_account.game_state {
            GameState::Player1Turn if payer_key == game_account.player_1 => {
                (true, game_account.player_1_arcium_pubkey)
            }
            GameState::Player2Turn if payer_key == game_account.player_2 => {
                (false, game_account.player_2_arcium_pubkey)
            }
            _ => return Err(BattleShipErrorCode::InvalidTurn.into()),
        };

        // Arguments for the `take_turn` encrypted instruction.
        let args = vec![
            // 1. Arguments for `Enc<Shared, TakeTurnInputs>`
            Argument::ArcisPubkey(player_arcium_pubkey),
            Argument::PlaintextU128(input_nonce),
            Argument::EncryptedU8(encrypted_guess), // this needs to be encrypted u32
            // 2. Arguments for `Enc<Mxe, PlayerShipFleet>`
            Argument::PlaintextU128(u128::from_le_bytes(game_account.fleet_state_nonce)),
            Argument::Account(game_account.key(), 8 + 148, 1088), // key, data offset, data size
        ];

        queue_computation(
            ctx.accounts,
            computation_offset,
            args,
            vec![CallbackAccount {
                pubkey: game_account_key,
                is_writable: true,
            }],
            None,
        )?;

        Ok(())
    }

    #[arcium_callback(encrypted_ix = "take_turn")]
    pub fn take_turn_callback(
        ctx: Context<TakeTurnCallback>,
        output: ComputationOutputs,
    ) -> Result<()> {
        let bytes = if let ComputationOutputs::Bytes(bytes) = output {
            bytes
        } else {
            return Err(BattleShipErrorCode::AbortedComputation.into());
        };

        let was_hit = bytes[0] == 1;
        let game_account = &mut ctx.accounts.game_account;

        // Check the turn to know whose ship count to decrement.
        let was_player_1_turn = game_account.game_state == GameState::Player1Turn;

        if was_hit {
            if was_player_1_turn {
                game_account.player_2_ships_left -= 1;
                if game_account.player_2_ships_left == 0 {
                    game_account.game_state = GameState::Finished;
                }
            } else {
                game_account.player_1_ships_left -= 1;
                if game_account.player_1_ships_left == 0 {
                    game_account.game_state = GameState::Finished;
                }
            }
        }

        // Advance the turn if the game is not over.
        if game_account.game_state != GameState::Finished {
            game_account.game_state = if was_player_1_turn {
                GameState::Player2Turn
            } else {
                GameState::Player1Turn
            };
        }

        emit!(TurnResult {
            game_pda: game_account.key(),
            was_hit,
            ships_left_player_1: game_account.player_1_ships_left,
            ships_left_player_2: game_account.player_2_ships_left,
            new_game_state: game_account.game_state,
        });

        Ok(())
    }
}

#[queue_computation_accounts("init_player_ship_fleet_location", payer)]
#[derive(Accounts)]
#[instruction(computation_offset: u64)]
pub struct NewGame<'info> {
    // the game pda creator, player1
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Account<'info, PersistentMXEAccount>,
    #[account(
        mut,
        address = derive_mempool_pda!()
    )]
    /// CHECK: mempool_account, checked by the arcium program.
    pub mempool_account: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_execpool_pda!()
    )]
    /// CHECK: executing_pool, checked by the arcium program.
    pub executing_pool: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_comp_pda!(computation_offset)
    )]
    /// CHECK: computation_account, checked by the arcium program.
    pub computation_account: UncheckedAccount<'info>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_INIT_PLAYER_SHIPS) // for mxe cluster to access mxe bytecode and metadata
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(
        mut,
        address = derive_cluster_pda!(mxe_account)
    )]
    pub cluster_account: Account<'info, Cluster>,
    #[account(
        mut,
        address = ARCIUM_STAKING_POOL_ACCOUNT_ADDRESS,
    )]
    pub pool_account: Account<'info, StakingPoolAccount>,
    #[account(
        address = ARCIUM_CLOCK_ACCOUNT_ADDRESS
    )]
    pub clock_account: Account<'info, ClockAccount>,
    pub system_program: Program<'info, System>,
    pub arcium_program: Program<'info, Arcium>,
    // game account
    #[account(
        init,
        payer = payer,
        space = 8 + GameData::INIT_SPACE,
        seeds = [b"game_data_account"],
        bump,
    )]
    pub game_account: Account<'info, GameData>,
}

#[callback_accounts("init_player_ship_fleet_location", payer)]
#[derive(Accounts)]
pub struct InitPlayerShipFleetLocCallback<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub arcium_program: Program<'info, Arcium>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_INIT_PLAYER_SHIPS)
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(address = ::anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: instructions_sysvar, checked by the account constraint
    pub instructions_sysvar: AccountInfo<'info>,
    /// CallBack account
    #[account(mut)]
    pub game_account: Account<'info, GameData>,
}

#[init_computation_definition_accounts("init_player_ship_fleet_location", payer)]
#[derive(Accounts)]
pub struct InitNewGameCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Box<Account<'info, PersistentMXEAccount>>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program.
    /// Can't check it here as it's not initialized yet.
    pub comp_def_account: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

#[queue_computation_accounts("place_ships", payer)]
#[derive(Accounts)]
#[instruction(computation_offset: u64)]
pub struct PlaceShipsIx<'info> {
    // the game pda creator, player1
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Account<'info, PersistentMXEAccount>,
    #[account(
        mut,
        address = derive_mempool_pda!()
    )]
    /// CHECK: mempool_account, checked by the arcium program.
    pub mempool_account: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_execpool_pda!()
    )]
    /// CHECK: executing_pool, checked by the arcium program.
    pub executing_pool: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_comp_pda!(computation_offset)
    )]
    /// CHECK: computation_account, checked by the arcium program.
    pub computation_account: UncheckedAccount<'info>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_PLACE_SHIPS) // for mxe cluster to access mxe bytecode and metadata
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(
        mut,
        address = derive_cluster_pda!(mxe_account)
    )]
    pub cluster_account: Account<'info, Cluster>,
    #[account(
        mut,
        address = ARCIUM_STAKING_POOL_ACCOUNT_ADDRESS,
    )]
    pub pool_account: Account<'info, StakingPoolAccount>,
    #[account(
        address = ARCIUM_CLOCK_ACCOUNT_ADDRESS
    )]
    pub clock_account: Account<'info, ClockAccount>,
    pub system_program: Program<'info, System>,
    pub arcium_program: Program<'info, Arcium>,
    // game account
    #[account(
        mut,
        seeds = [b"game_data_account"],
        bump = game_account.bump,
    )]
    pub game_account: Account<'info, GameData>,
}

#[callback_accounts("place_ships", payer)]
#[derive(Accounts)]
pub struct PlaceShipsIxCallback<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub arcium_program: Program<'info, Arcium>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_PLACE_SHIPS)
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(address = ::anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: instructions_sysvar, checked by the account constraint
    pub instructions_sysvar: AccountInfo<'info>,
    /// encrypted instruction cb
    #[account(mut)]
    pub game_account: Account<'info, GameData>,
}

#[init_computation_definition_accounts("place_ships", payer)]
#[derive(Accounts)]
pub struct InitPlaceShipsCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Box<Account<'info, PersistentMXEAccount>>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program.
    /// Can't check it here as it's not initialized yet.
    pub comp_def_account: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

#[queue_computation_accounts("take_turn", payer)]
#[derive(Accounts)]
#[instruction(computation_offset: u64)]
pub struct TakeTurnIx<'info> {
    // the game pda creator, player1
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Account<'info, PersistentMXEAccount>,
    #[account(
        mut,
        address = derive_mempool_pda!()
    )]
    /// CHECK: mempool_account, checked by the arcium program.
    pub mempool_account: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_execpool_pda!()
    )]
    /// CHECK: executing_pool, checked by the arcium program.
    pub executing_pool: UncheckedAccount<'info>,
    #[account(
        mut,
        address = derive_comp_pda!(computation_offset)
    )]
    /// CHECK: computation_account, checked by the arcium program.
    pub computation_account: UncheckedAccount<'info>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_TAKE_TURN) // for mxe cluster to access mxe bytecode and metadata
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(
        mut,
        address = derive_cluster_pda!(mxe_account)
    )]
    pub cluster_account: Account<'info, Cluster>,
    #[account(
        mut,
        address = ARCIUM_STAKING_POOL_ACCOUNT_ADDRESS,
    )]
    pub pool_account: Account<'info, StakingPoolAccount>,
    #[account(
        address = ARCIUM_CLOCK_ACCOUNT_ADDRESS
    )]
    pub clock_account: Account<'info, ClockAccount>,
    pub system_program: Program<'info, System>,
    pub arcium_program: Program<'info, Arcium>,
    // game account
    #[account(
        mut,
        seeds = [b"game_data_account"],
        bump = game_account.bump,
    )]
    pub game_account: Account<'info, GameData>,
}

#[callback_accounts("take_turn", payer)]
#[derive(Accounts)]
pub struct TakeTurnCallback<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub arcium_program: Program<'info, Arcium>,
    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_TAKE_TURN)
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,
    #[account(address = ::anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: instructions_sysvar, checked by the account constraint
    pub instructions_sysvar: AccountInfo<'info>,
    // encrypted ix callback
    #[account(mut)]
    pub game_account: Account<'info, GameData>,
}

#[init_computation_definition_accounts("take_turn", payer)]
#[derive(Accounts)]
pub struct InitTakeTurnCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Box<Account<'info, PersistentMXEAccount>>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program.
    /// Can't check it here as it's not initialized yet.
    pub comp_def_account: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct GameData {
    pub bump: u8,                         // 1
    pub player_1: Pubkey,                 // 32
    pub player_2: Pubkey,                 // 32
    pub player_1_arcium_pubkey: [u8; 32], // Arcis pubkey for client-side encryption
    pub player_2_arcium_pubkey: [u8; 32],
    pub game_state: GameState,   // 1
    pub player_1_ships_left: u8, // 1
    pub player_2_ships_left: u8, // 1

    // Nonce for mxe to decrypt ships
    pub fleet_state_nonce: [u8; 16], // 16
    // each player have 17 possible locations on the matrix of 10*10
    // each location will be of `[u8; 32] - cipher text`
    // so we can store the [[u8; 32]; 17+17] in a single state
    pub encrypted_fleet_state: [[u8; 32]; 34],
}

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameState {
    PlacingShips = 0,
    Player1Turn = 1,
    Player2Turn = 2,
    Finished = 3,
}

#[error_code]
pub enum BattleShipErrorCode {
    #[msg("The computation was aborted by the Arcium network.")]
    AbortedComputation,
    #[msg("The game is not in the correct state for this action.")]
    InvalidGameState,
    #[msg("It is not currently this player's turn.")]
    InvalidTurn,
    #[msg("The transaction was signed by an unauthorized player.")]
    UnauthorizedPlayer,
}

#[event]
pub struct GameCreated {
    game_pda: Pubkey,
    player_1: Pubkey,
    player_2: Pubkey,
}

#[event]
pub struct ShipsPlaced {
    game_pda: Pubkey,
}

#[event]
pub struct TurnResult {
    game_pda: Pubkey,
    was_hit: bool,
    ships_left_player_1: u8,
    ships_left_player_2: u8,
    new_game_state: GameState,
}
