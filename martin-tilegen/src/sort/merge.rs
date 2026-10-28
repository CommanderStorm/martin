use std::{mem, thread};

use tracing::warn;

use super::run::{Run, RunReader};
use crate::TileGenResult;
use crate::sort::SortKey;

/// Streams the records of several runs in key order. Equal keys come out in run order,
/// because the readers are kept in run-index order and ties are broken by reader position.
pub struct Merger {
    readers: Vec<RunReader>,
    tree: LoserTree,
}

impl Merger {
    pub(super) fn new(runs: Vec<Run>) -> TileGenResult<Self> {
        let readers = runs
            .into_iter()
            .map(RunReader::open)
            .collect::<TileGenResult<Vec<_>>>()?;
        let tree = LoserTree::new(readers.iter().map(head_key));
        Ok(Self { readers, tree })
    }

    /// The next record; it stays borrowable until [`advance`](Self::advance).
    #[must_use]
    pub fn peek(&self) -> Option<(SortKey, &[u8])> {
        let reader = &self.readers[self.tree.winner()?];
        reader.head().map(|key| (key, reader.record()))
    }

    /// Moves past the record [`peek`](Self::peek) returns.
    pub fn advance(&mut self) -> TileGenResult<()> {
        if let Some(winner) = self.tree.winner() {
            let reader = &mut self.readers[winner];
            reader.advance()?;
            self.tree.replay(winner, head_key(reader));
        }
        Ok(())
    }
}

impl Drop for Merger {
    /// Unmapping a temp file frees its pages, seconds for a large merge, so it happens off the merging thread.
    fn drop(&mut self) {
        let readers = mem::take(&mut self.readers);
        if readers.is_empty() {
            return;
        }
        if let Err(err) = thread::Builder::new()
            .name("run-close".to_owned())
            .spawn(move || drop(readers))
        {
            warn!("closed sort runs on the merging thread, no thread could be started: {err}");
        }
    }
}

/// An exhausted reader's key, after every real key: a tile id never fills the top byte of its word.
const EXHAUSTED: (u64, u64) = (u64::MAX, u64::MAX);

fn head_key(reader: &RunReader) -> (u64, u64) {
    reader.head().map_or(EXHAUSTED, SortKey::words)
}

/// A leaf's current key, compared before its index, so ties go to the earlier run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Node {
    key: (u64, u64),
    leaf: u32,
}

/// Tournament tree over `k` leaves: `nodes[0]` is the winner, `nodes[1..k]` the losers of each match,
/// so replacing the winner costs one comparison per level instead of a heap's two. Nodes hold their
/// keys, so a replay compares within the tree and never reads the runs.
struct LoserTree {
    nodes: Vec<Node>,
}

impl LoserTree {
    /// Marks a match no leaf has reached yet, only while building.
    const EMPTY: u32 = u32::MAX;

    fn new(keys: impl ExactSizeIterator<Item = (u64, u64)>) -> Self {
        let empty = Node {
            key: EXHAUSTED,
            leaf: Self::EMPTY,
        };
        let mut tree = Self {
            nodes: vec![empty; keys.len()],
        };
        for (leaf, key) in keys.enumerate() {
            tree.replay(leaf, key);
        }
        tree
    }

    fn winner(&self) -> Option<usize> {
        self.nodes.first().map(|node| node.leaf as usize)
    }

    /// While building, the first leaf to reach an empty match waits there for its opponent.
    fn replay(&mut self, leaf: usize, key: (u64, u64)) {
        let k = self.nodes.len();
        #[expect(clippy::cast_possible_truncation, reason = "runs are numbered by a u32")]
        let mut winner = Node { key, leaf: leaf as u32 };
        let mut node = usize::midpoint(leaf, k);
        while node > 0 {
            let other = self.nodes[node];
            if other.leaf == Self::EMPTY {
                self.nodes[node] = winner;
                return;
            }
            if other < winner {
                self.nodes[node] = winner;
                winner = other;
            }
            node /= 2;
        }
        self.nodes[0] = winner;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loser_tree_yields_sorted_order_for_any_width() {
        for k in 1..=9_u64 {
            let mut heads: Vec<Vec<u64>> = (0..k).map(|i| (0..5).map(|j| j * 3 + i % 3).rev().collect()).collect();
            let key = |heads: &[Vec<u64>], i: usize| heads[i].last().map_or(EXHAUSTED, |&v| (v, 0));
            let mut tree = LoserTree::new((0..heads.len()).map(|i| key(&heads, i)));
            let mut out = Vec::new();
            while let Some(w) = tree.winner().filter(|&w| heads[w].last().is_some()) {
                out.push((heads[w].pop().unwrap(), w));
                tree.replay(w, key(&heads, w));
            }
            let mut expected = out.clone();
            expected.sort_unstable();
            assert_eq!(out, expected, "k={k}");
            assert_eq!(out.len() as u64, 5 * k);
        }
    }
}
