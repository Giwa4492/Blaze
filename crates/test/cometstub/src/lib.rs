//! `penumbra-cometstub` is an in-memory consensus engine for integration tests.
//
//  TODO(kate):
//  - `tests/ibc_handshake.rs` contains a starter test case to build out.

pub mod genesis;
pub mod validator;

mod abci;
mod header;
mod state;

use {
    self::{
        genesis::Genesis,
        validator::{Validator, Validators},
    },
    tendermint::{block, chain, AppHash, Hash, Time},
};

/// An in-memory consensus engine for integration tests.
///
/// See the crate-level documentation for more information.
pub struct Engine {
    /// Inner consensus state.
    #[allow(dead_code)] // XXX(kate)
    state: State,
    /// The last block that has been generated by the consensus engine.
    //
    //  TODO(kate): bikeshed this later.
    pub block: tendermint::Block,
}

/// Consensus state used by the [`Engine`] to generate [`Block`s][tendermint::Block].
//
//  TODO(kate): make this `pub(crate)`. use Engine as the public layer for the caller.
#[allow(dead_code)] // XXX(kate)
pub struct State {
    /// The chain identifier.
    chain_id: chain::Id,
    /// The initial [`block::Height`].
    initial_height: block::Height,
    /// Metadata regarding the last block generated.
    last_block: Option<LastBlock>,
    /// The set of validators.
    validators: Validators,
    /// The latest app hash.
    app_hash: AppHash,
    /// The merkle root of the result of executing the previous block.
    last_results_hash: Option<Hash>,
    // TODO(kate): handle consensus parameters later.
    // consensus_params: (),
    // last_height_consensus_params_changed: block::Height,
}

/// Consensus [`State`] metadata regarding the last block generated.
#[allow(dead_code)] // XXX(kate)
pub struct LastBlock {
    height: block::Height,
    id: block::Id,
    time: Time,
}

// === impl Engine ===

impl Engine {
    /// Returns a new [`Engine`].
    //
    //  TODO(kate): for now, use a default `Genesis` structure. add a constructor receiving a
    //  `State` at some point later.
    pub fn new() -> Self {
        let (state, block) = Genesis::default().into_state();
        Self { state, block }
    }

    pub fn next_block(&self) -> tendermint::Block {
        State::generate_block()
    }

    pub fn chain_id(&self) -> &chain::Id {
        &self.state.chain_id
    }

    // TODO(kate): consider amortizing this at some point later on.
    pub fn validators(&self) -> tendermint::validator::Set {
        let info = self
            .state
            .validators
            .current
            .iter()
            .map(Validator::info)
            .collect();
        tendermint::validator::Set::without_proposer(info)
    }
}

// === impl State ===

impl State {
    pub(crate) fn generate_block() -> tendermint::Block {
        let header = self::header::header();
        let data = vec![]; // tx's
        let evidence = tendermint::evidence::List::default();
        let last_commit = None;
        let block = tendermint::Block::new(
            header,
            data,
            evidence,
            last_commit,
        ).unwrap(/*XXX(kate): handle invalid block errors*/);

        block
    }
}
