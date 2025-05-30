use std::{collections::HashMap, fmt};

use valence_nbt::Value;

use crate::escape_nbt_string;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemKey {
    pub id: String,
    pub components_snbt: Option<String>,
}

impl ItemKey {
    pub fn new(id: String, components_nbt: Option<&Value>) -> Self {
        ItemKey {
            id,
            components_snbt: components_nbt.map(valence_nbt::snbt::to_snbt_string),
        }
    }
}

impl fmt::Display for ItemKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.components_snbt {
            Some(snbt) => write!(f, "{} {}", self.id, escape_nbt_string(snbt)),
            None => write!(f, "{}", self.id),
        }
    }
}

pub struct Counter {
    counts: HashMap<ItemKey, u64>,
}

impl Counter {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: String, components_nbt: Option<&Value>, count: u64) {
        let key = ItemKey::new(id, components_nbt);
        *self.counts.entry(key).or_insert(0) += count;
    }

    pub fn is_empty(&self) -> bool {
        self.counts.iter().all(|(_, &count)| count == 0)
    }

    pub fn merge(&mut self, other: &Self) {
        for (key, &count) in other.detailed_counts() {
            *self.counts.entry(key.clone()).or_insert(0) += count;
        }
    }

    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    pub fn total_by_id(&self) -> HashMap<String, u64> {
        let mut totals = HashMap::new();
        for (key, &count) in &self.counts {
            *totals.entry(key.id.clone()).or_insert(0) += count;
        }
        totals
    }

    pub fn total_by_nbt(&self) -> HashMap<Option<String>, u64> {
        let mut m = HashMap::new();
        for (key, &cnt) in &self.counts {
            *m.entry(key.components_snbt.clone()).or_insert(0) += cnt;
        }
        m
    }

    pub fn detailed_counts(&self) -> &HashMap<ItemKey, u64> {
        &self.counts
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}
