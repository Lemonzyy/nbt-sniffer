use ptree::{Style, TreeItem};
use std::{borrow::Cow, collections::HashMap, fmt, io};

#[derive(Debug, Clone)]
pub enum ItemSummaryNode {
    Root {
        label: String,
        children: Vec<ItemSummaryNode>,
    },
    Item {
        id: String,
        count: u64,
        snbt: Option<String>,
        children: Vec<ItemSummaryNode>,
    },
}

impl ItemSummaryNode {
    pub fn new_root(label: String, children: Vec<ItemSummaryNode>) -> Self {
        ItemSummaryNode::Root { label, children }
    }

    pub fn new_item(
        id: String,
        count: u64,
        snbt: Option<String>,
        children: Vec<ItemSummaryNode>,
    ) -> Self {
        ItemSummaryNode::Item {
            id,
            count,
            snbt,
            children,
        }
    }

    fn children_mut(&mut self) -> &mut Vec<ItemSummaryNode> {
        match self {
            ItemSummaryNode::Root { children, .. } => children,
            ItemSummaryNode::Item { children, .. } => children,
        }
    }

    /// Collapse all direct children that are leaf‐nodes with identical `(id, snbt)`,
    /// summing their `count`. Then, recurse into any child that still has its own children.
    ///
    /// After you call this on a node, you will guarantee that:
    /// - At this node's level, no two `Item { … }` leaf nodes share the same `id` and `snbt`.
    /// - All interior (non‐leaf) children have themselves had `collapse_leaves_recursive` called on them,
    ///   so the entire subtree is cleanly collapsed.
    pub fn collapse_leaves_recursive(&mut self) {
        let mut leaf_map = HashMap::new();
        let mut new_children = Vec::new();

        for child in self.children_mut().drain(..) {
            if let ItemSummaryNode::Item {
                id,
                count,
                snbt,
                children,
            } = &child
                && children.is_empty()
            {
                let key = (id.clone(), snbt.clone());
                *leaf_map.entry(key).or_default() += *count;
                continue;
            }

            new_children.push(child);
        }

        for ((id, snbt), total_count) in leaf_map.into_iter() {
            let merged_leaf = ItemSummaryNode::new_item(id, total_count, snbt, Vec::new());
            new_children.push(merged_leaf);
        }

        new_children.sort_by(|a, b| {
            let (a_count, a_id) = match a {
                ItemSummaryNode::Item { count, id, .. } => (*count, id.clone()),
                ItemSummaryNode::Root { label, .. } => (0u64, label.clone()),
            };
            let (b_count, b_id) = match b {
                ItemSummaryNode::Item { count, id, .. } => (*count, id.clone()),
                ItemSummaryNode::Root { label, .. } => (0u64, label.clone()),
            };
            b_count.cmp(&a_count).then(a_id.cmp(&b_id))
        });

        *self.children_mut() = new_children;

        for child in self.children_mut().iter_mut() {
            if let ItemSummaryNode::Item { children, .. } = child
                && !children.is_empty()
            {
                child.collapse_leaves_recursive();
            }
        }
    }
}

impl fmt::Display for ItemSummaryNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemSummaryNode::Root { label, .. } => write!(f, "{label}"),
            ItemSummaryNode::Item {
                id, count, snbt, ..
            } => {
                let count_part = if *count > 0 {
                    format!("{count}x ")
                } else {
                    String::new()
                };
                let snbt_part = match snbt {
                    Some(snbt) => format!(" {snbt}"),
                    None => String::new(),
                };

                write!(f, "{count_part}{id}{snbt_part}")
            }
        }
    }
}

impl TreeItem for ItemSummaryNode {
    type Child = ItemSummaryNode;

    fn write_self<W: io::Write>(&self, f: &mut W, _style: &Style) -> io::Result<()> {
        write!(f, "{self}")
    }

    fn children(&self) -> std::borrow::Cow<'_, [Self::Child]> {
        match self {
            ItemSummaryNode::Root { children, .. } => Cow::Borrowed(children),
            ItemSummaryNode::Item { children, .. } => Cow::Borrowed(children),
        }
    }
}
