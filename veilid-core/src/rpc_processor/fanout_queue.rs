use super::*;

pub struct FanoutQueue {
    crypto_kind: CryptoKind,
    current_nodes: VecDeque<NodeRef>,
    returned_nodes: HashSet<TypedKey>,
}

impl FanoutQueue {
    // Create a queue for fanout candidates that have a crypto-kind compatible node id
    pub fn new(crypto_kind: CryptoKind) -> Self {
        Self {
            crypto_kind,
            current_nodes: VecDeque::new(),
            returned_nodes: HashSet::new(),
        }
    }

    // Add new nodes to list of fanout candidates
    // Run a cleanup routine afterwards to trim down the list of candidates so it doesn't grow too large
    pub fn add<F: FnOnce(&[NodeRef]) -> Vec<NodeRef>>(
        &mut self,
        new_nodes: &[NodeRef],
        cleanup: F,
    ) {
        for nn in new_nodes {
            // Ensure the node has a comparable key with our current crypto kind
            let Some(key) = nn.node_ids().get(self.crypto_kind) else {
                continue;
            };
            // Check if we have already done this node before (only one call per node ever)
            if self.returned_nodes.contains(&key) {
                continue;
            }

            // Make sure the new node isnt already in the list
            let mut dup = false;
            for cn in &self.current_nodes {
                if cn.same_entry(nn) {
                    dup = true;
                    break;
                }
            }
            if !dup {
                // Add the new node
                self.current_nodes.push_front(nn.clone());
            }
        }

        // Make sure the deque is a single slice
        self.current_nodes.make_contiguous();

        // Sort and trim the candidate set
        self.current_nodes =
            VecDeque::from_iter(cleanup(self.current_nodes.as_slices().0).iter().cloned());
    }

    // Return next fanout candidate
    pub fn next(&mut self) -> Option<NodeRef> {
        let cn = self.current_nodes.pop_front()?;
        self.current_nodes.make_contiguous();
        let key = cn.node_ids().get(self.crypto_kind).unwrap();

        // Ensure we don't return this node again
        self.returned_nodes.insert(key);
        Some(cn)
    }

    // Get a slice of all the current fanout candidates
    pub fn nodes(&self) -> &[NodeRef] {
        self.current_nodes.as_slices().0
    }
}