use std::{collections::HashMap, fmt};

use valence_nbt::Value;

use crate::{Scope, escape_nbt_string};

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

#[derive(Debug, Clone)]
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

#[derive(Debug)]
pub struct CounterMap {
    map: HashMap<Scope, Counter>,
}

impl CounterMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn entry_counter(&mut self, scope: Scope) -> &mut Counter {
        self.map.entry(scope).or_default()
    }

    pub fn merge_scope(&mut self, scope: Scope, other: &Counter) {
        self.entry_counter(scope).merge(other);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Scope, &Counter)> {
        self.map.iter()
    }

    pub fn combined(&self) -> Counter {
        let mut combined = Counter::new();
        for counter in self.map.values() {
            combined.merge(counter);
        }
        combined
    }
}

impl Default for CounterMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::DataType;

    use super::*;
    use valence_nbt::Value;

    fn nbt_val(s: &str) -> Value {
        valence_nbt::snbt::from_snbt_str(s).unwrap()
    }

    #[test]
    fn item_key_new_and_display() {
        let key1 = ItemKey::new("minecraft:diamond".to_string(), None);
        assert_eq!(key1.id, "minecraft:diamond");
        assert!(key1.components_snbt.is_none());
        assert_eq!(format!("{}", key1), "minecraft:diamond");

        let nbt = nbt_val("{components:{\"minecraft:enchantments\":{\"minecraft:sharpness\":1s}}}");
        let key2 = ItemKey::new("minecraft:sword".to_string(), Some(&nbt));
        assert_eq!(key2.id, "minecraft:sword");
        assert!(key2.components_snbt.is_some());
        assert_eq!(
            format!("{}", key2),
            "minecraft:sword {components:{\"minecraft:enchantments\":{\"minecraft:sharpness\":1s}}}"
        );
    }

    #[test]
    fn counter_add_and_total() {
        let mut counter = Counter::new();
        assert_eq!(counter.total(), 0);

        counter.add("minecraft:dirt".to_string(), None, 64);
        assert_eq!(counter.total(), 64);

        let nbt = nbt_val("{components:{\"minecraft:damage\":10}}");
        counter.add("minecraft:pickaxe".to_string(), Some(&nbt), 1);
        assert_eq!(counter.total(), 65);

        counter.add("minecraft:dirt".to_string(), None, 32); // Add to existing
        assert_eq!(counter.total(), 97);
        assert_eq!(counter.detailed_counts().len(), 2);
    }

    #[test]
    fn counter_merge() {
        let mut counter1 = Counter::new();
        counter1.add("minecraft:stone".to_string(), None, 10);
        let nbt = nbt_val("{CustomTag:1b}");
        counter1.add("minecraft:chest".to_string(), Some(&nbt), 5);

        let mut counter2 = Counter::new();
        counter2.add("minecraft:stone".to_string(), None, 20); // Overlapping key
        counter2.add("minecraft:wood".to_string(), None, 15); // New key

        counter1.merge(&counter2);

        assert_eq!(counter1.total(), 10 + 5 + 20 + 15);
        assert_eq!(counter1.detailed_counts().len(), 3);

        let key_stone = ItemKey::new("minecraft:stone".to_string(), None);
        assert_eq!(counter1.detailed_counts().get(&key_stone), Some(&30));

        let key_chest = ItemKey::new("minecraft:chest".to_string(), Some(&nbt));
        assert_eq!(counter1.detailed_counts().get(&key_chest), Some(&5));

        let key_wood = ItemKey::new("minecraft:wood".to_string(), None);
        assert_eq!(counter1.detailed_counts().get(&key_wood), Some(&15));
    }

    #[test]
    fn counter_total_by_id() {
        let mut counter = Counter::new();
        let nbt1 = nbt_val("{tag:\"a\"}");
        let nbt2 = nbt_val("{tag:\"b\"}");
        counter.add("item_A".to_string(), None, 10);
        counter.add("item_A".to_string(), Some(&nbt1), 5);
        counter.add("item_B".to_string(), Some(&nbt2), 20);

        let totals_by_id = counter.total_by_id();
        assert_eq!(totals_by_id.get("item_A"), Some(&15));
        assert_eq!(totals_by_id.get("item_B"), Some(&20));
        assert_eq!(totals_by_id.len(), 2);
    }

    #[test]
    fn counter_total_by_nbt() {
        let mut counter = Counter::new();
        let nbt_a_str = "{tag:\"a\"}";
        let nbt_a_val = nbt_val(nbt_a_str);
        let nbt_b_str = "{tag:\"b\"}";
        let nbt_b_val = nbt_val(nbt_b_str);

        // Get the canonical SNBT string as valence_nbt would produce it
        let nbt_a_canonical_snbt = valence_nbt::snbt::to_snbt_string(&nbt_a_val);
        let nbt_b_canonical_snbt = valence_nbt::snbt::to_snbt_string(&nbt_b_val);

        counter.add("item1".to_string(), Some(&nbt_a_val), 10);
        counter.add("item2".to_string(), Some(&nbt_a_val), 5); // Same NBT, different ID
        counter.add("item3".to_string(), Some(&nbt_b_val), 20);
        counter.add("item4".to_string(), None, 7);

        let totals_by_nbt = counter.total_by_nbt();
        assert_eq!(totals_by_nbt.get(&Some(nbt_a_canonical_snbt)), Some(&15));
        assert_eq!(totals_by_nbt.get(&Some(nbt_b_canonical_snbt)), Some(&20));
        assert_eq!(totals_by_nbt.get(&None), Some(&7));
        assert_eq!(totals_by_nbt.len(), 3);
    }

    #[test]
    fn counter_map_entry_and_merge_scope() {
        let mut map = CounterMap::new();
        let scope1 = Scope {
            dimension: "overworld".to_string(),
            data_type: DataType::BlockEntity,
        };
        let scope2 = Scope {
            dimension: "nether".to_string(),
            data_type: DataType::Entity,
        };

        map.entry_counter(scope1.clone())
            .add("minecraft:cobblestone".to_string(), None, 100);
        assert_eq!(map.map.get(&scope1).unwrap().total(), 100);

        let mut other_counter = Counter::new();
        other_counter.add("minecraft:blaze_rod".to_string(), None, 10);
        map.merge_scope(scope2.clone(), &other_counter);
        assert_eq!(map.map.get(&scope2).unwrap().total(), 10);

        // Merge into existing scope
        let mut another_counter = Counter::new();
        another_counter.add("minecraft:cobblestone".to_string(), None, 50);
        map.merge_scope(scope1.clone(), &another_counter);
        assert_eq!(map.map.get(&scope1).unwrap().total(), 150);
    }

    #[test]
    fn counter_map_combined() {
        let mut map = CounterMap::new();
        let scope1 = Scope {
            dimension: "overworld".to_string(),
            data_type: DataType::BlockEntity,
        };
        let scope2 = Scope {
            dimension: "overworld".to_string(),
            data_type: DataType::Entity,
        };

        map.entry_counter(scope1.clone())
            .add("A".to_string(), None, 10);
        map.entry_counter(scope1.clone())
            .add("B".to_string(), None, 20); // Total 30 for scope1

        map.entry_counter(scope2.clone())
            .add("A".to_string(), None, 5); // Total 5 for scope2
        map.entry_counter(scope2.clone())
            .add("C".to_string(), None, 15); // Total 15 for scope2

        let combined = map.combined();
        assert_eq!(combined.total(), 10 + 20 + 5 + 15);
        let totals_by_id = combined.total_by_id();
        assert_eq!(totals_by_id.get("A"), Some(&15));
        assert_eq!(totals_by_id.get("B"), Some(&20));
        assert_eq!(totals_by_id.get("C"), Some(&15));
    }
}
