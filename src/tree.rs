use ptree::{Style, TreeItem};
use std::{borrow::Cow, fmt, io};

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
    pub fn new_root(label: impl Into<String>, children: Vec<ItemSummaryNode>) -> Self {
        ItemSummaryNode::Root {
            label: label.into(),
            children,
        }
    }

    pub fn new_item(
        id: impl Into<String>,
        count: u64,
        snbt: Option<String>,
        children: Vec<ItemSummaryNode>,
    ) -> Self {
        ItemSummaryNode::Item {
            id: id.into(),
            count,
            snbt,
            children,
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

    fn children(&self) -> std::borrow::Cow<[Self::Child]> {
        match self {
            ItemSummaryNode::Root { children, .. } => Cow::Borrowed(children),
            ItemSummaryNode::Item { children, .. } => Cow::Borrowed(children),
        }
    }
}
