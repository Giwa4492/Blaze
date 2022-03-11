use std::ops::Deref;

use hash_hasher::HashedMap;

use crate::*;

#[path = "block.rs"]
mod block;
pub use block::{Block, BlockMut};

/// A sparse commitment tree to witness up to 65,536 [`Block`]s, each witnessing up to 65,536 [`Fq`]s
/// or their [`struct@Hash`]es.
///
/// This is one [`Epoch`] in an [`Eternity`].
#[derive(Derivative, Debug, Clone, PartialEq, Eq, Default)]
pub struct Epoch {
    pub(super) block_index: HashedMap<Fq, u16>,
    pub(super) item_index: HashedMap<Fq, u16>,
    pub(super) inner: Tier<Tier<Item>>,
}

/// A mutable reference to an [`Epoch`] within an [`Eternity`](super::Eternity).
///
/// This supports all the methods of [`Epoch`] that take `&mut self` or `&self`.
pub struct EpochMut<'a> {
    pub(super) super_index: Option<(u16, &'a mut HashedMap<Fq, u16>)>,
    epoch: &'a mut Epoch,
}

/// [`EpochMut`] implements `Deref<Target = Epoch>` so it inherits all the *immutable* methods from
/// [`Epoch`], but crucially it *does not* implemennt `DerefMut`, because the *mutable* methods in
/// `Epoch` are defined in terms of methods on `EpochMut`.
impl Deref for EpochMut<'_> {
    type Target = Epoch;

    fn deref(&self) -> &Self::Target {
        &*self.epoch
    }
}

impl Height for Epoch {
    type Height = <Tier<Tier<Item>> as Height>::Height;
}

impl Epoch {
    /// Create a new empty [`Epoch`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Get an [`EpochMut`] referring to this [`Epoch`].
    pub fn as_mut(&mut self) -> EpochMut {
        EpochMut {
            super_index: None,
            epoch: self,
        }
    }

    /// Add a new [`Block`] or its root [`struct@Hash`] all at once to this [`Epoch`].
    ///
    /// # Errors
    ///
    /// Returns `Err(block)` containing the inserted block without adding it to the [`Epoch`] if the
    /// [`Epoch`] is full.
    pub fn insert(&mut self, block: Insert<Block>) -> Result<(), Insert<Block>> {
        self.as_mut().insert(block)
    }

    /// The total number of [`Fq`]s or [`struct@Hash`]es represented in this [`Epoch`].
    ///
    /// This count includes those which were elided due to a partially filled [`Block`] or summary
    /// root [`struct@Hash`] of a block being inserted.
    ///
    /// In other words, this is `4 ^ 8` times the number of blocks represented in this [`Epoch`],
    /// plus the number of items in the latest block.
    ///
    /// The maximum capacity of an [`Epoch`] is `2 ^ 32`, i.e. `4 ^ 8` blocks of `4 ^ 8` items.
    pub fn len(&self) -> u32 {
        ((self.inner.len() as u32) << 16)
            + match self.inner.focus() {
                None => 0,
                Some(Insert::Hash(_)) => u16::MAX,
                Some(Insert::Keep(block)) => block.len(),
            } as u32
    }

    /// Check whether this [`Epoch`] is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the root [`struct@Hash`] of this [`Epoch`].
    ///
    /// Internal hashing is performed lazily to prevent unnecessary intermediary hashes from being
    /// computed, so the first hash returned after a long sequence of insertions may take more time
    /// than subsequent calls.
    ///
    /// Computed hashes are cached so that subsequent calls without further modification are very
    /// fast.
    pub fn hash(&self) -> Hash {
        self.inner.hash()
    }

    /// Get a [`Proof`] of inclusion for the item at this index in the epoch.
    ///
    /// If the index is not witnessed in this epoch, return `None`.
    pub fn witness(&self, item: Fq) -> Option<Proof<Epoch>> {
        // Calculate the index for this item
        let block_in_epoch = *self.block_index.get(&item)?;
        let item_in_block = *self
            .item_index
            .get(&item)
            .expect("if item is present in block index, it must be present in item index");
        let index = ((block_in_epoch as usize) << 16) | item_in_block as usize;

        let (auth_path, leaf) = self.inner.witness(index)?;
        Some(Proof {
            index,
            auth_path,
            leaf,
        })
    }
}

impl EpochMut<'_> {
    /// Add a new [`Block`] or its root [`struct@Hash`] all at once to the underlying [`Epoch`]: see
    /// [`Epoch::insert`].
    pub fn insert(&mut self, block: Insert<Block>) -> Result<(), Insert<Block>> {
        // TODO: deal with duplicates

        // If we successfully insert this block, here's what its index in the epoch will be:
        let this_block = self.inner.len();

        // Strip out the item index of the block
        let (block, item_index) = match block {
            Insert::Hash(hash) => (Insert::Hash(hash), Default::default()),
            Insert::Keep(Block { item_index, inner }) => (Insert::Keep(inner), item_index),
        };

        // Try to insert the block into the tree, and if successful, track the item and block
        // indices of each item in the inserted block
        if let Err(block) = self.epoch.inner.insert(block) {
            Err(block.map(|inner| Block { item_index, inner }))
        } else {
            // If there is a super-index (i.e. we are one epoch of an extant eternity), then also
            // insert our own epoch index as the epoch index of each item in the block
            if let Some((this_epoch, epoch_index)) = &mut self.super_index {
                epoch_index.extend(item_index.iter().map(|(item, _)| (*item, *this_epoch)));
            }
            // Keep track of the block index of each item in the block (these are all the same, all
            // pointing to this block we just inserted)
            self.epoch
                .block_index
                .extend(item_index.iter().map(|(item, _)| (*item, this_block)));
            // Keep track of the index within its own block of each item in the block
            self.epoch.item_index.extend(item_index.iter());
            Ok(())
        }
    }
}
