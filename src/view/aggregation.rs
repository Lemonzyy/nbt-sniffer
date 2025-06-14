use crate::{
    DataType,
    counter::{Counter, CounterMap},
};
use std::collections::{BTreeMap, HashMap};

/// A helper trait to check if a summary data structure is empty.
pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl IsEmpty for Counter {
    fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

impl IsEmpty for CounterMap {
    fn is_empty(&self) -> bool {
        self.iter().all(|(_, counter)| counter.is_empty())
    }
}

impl<K, V> IsEmpty for HashMap<K, V> {
    fn is_empty(&self) -> bool {
        HashMap::is_empty(self)
    }
}

/// Trait for items that can be aggregated.
///
/// This trait abstracts the operations needed for different types of summary data,
/// such as creating an empty summary, deriving a summary from a base `Counter`,
/// and merging two summaries together.
pub trait Aggregable: Sized + Clone + IsEmpty {
    /// Creates a new, empty instance of the aggregable item.
    fn new_empty() -> Self;

    /// Creates an instance of the aggregable item from a `Counter`.
    fn from_counter(counter: &Counter) -> Self;

    /// Merges another instance of the aggregable item into this one.
    fn merge(&mut self, other: &Self);
}

impl Aggregable for Counter {
    fn new_empty() -> Self {
        Counter::new()
    }

    fn from_counter(counter: &Counter) -> Self {
        counter.clone()
    }

    fn merge(&mut self, other: &Self) {
        Counter::merge(self, other);
    }
}

impl Aggregable for HashMap<String, u64> {
    fn new_empty() -> Self {
        HashMap::new()
    }

    fn from_counter(counter: &Counter) -> Self {
        counter.total_by_id()
    }

    fn merge(&mut self, other: &Self) {
        for (key, value) in other {
            *self.entry(key.clone()).or_insert(0) += *value;
        }
    }
}

/// Generic struct to hold aggregated data results.
///
/// `T` is a type that implements the `Aggregable` trait, representing the
/// kind of summary item being aggregated (e.g., `Counter` or `HashMap<String, u64>`).
pub struct AggregationResult<T: Aggregable> {
    /// Data grouped by dimension and then by `DataType`.
    pub grouped: BTreeMap<String, BTreeMap<DataType, T>>,
    /// Totals for each `DataType` across all dimensions.
    pub total_by_type: BTreeMap<DataType, T>,
    /// Overall combined total across all dimensions and `DataType`s.
    pub total_combined: T,
}

impl<T: Aggregable> AggregationResult<T> {
    /// Creates a new `AggregationResult` by processing a `CounterMap`.
    pub fn new(counter_map: &CounterMap) -> Self {
        let mut grouped: BTreeMap<String, BTreeMap<DataType, T>> = BTreeMap::new();
        let mut total_by_type: BTreeMap<DataType, T> = BTreeMap::new();
        let mut total_combined = T::new_empty();

        for (scope, counter) in counter_map.iter() {
            // Skip processing if the counter is empty to avoid creating empty summary items unnecessarily.
            if counter.is_empty() {
                continue;
            }
            let item_summary = T::from_counter(counter);

            // Populate grouped data
            grouped
                .entry(scope.dimension.clone())
                .or_default()
                .entry(scope.data_type)
                .or_insert_with(T::new_empty)
                .merge(&item_summary);

            // Populate total_by_type
            total_by_type
                .entry(scope.data_type)
                .or_insert_with(T::new_empty)
                .merge(&item_summary);

            // Populate total_combined
            total_combined.merge(&item_summary);
        }

        // Ensure all DataType variants are present in total_by_type, initialized to empty if no data was found.
        // This makes the SummaryDataProvider accessors more reliable.
        // Assuming DataType enum has at least these variants.
        // If DataType had an iter_variants() or similar, that could be used here.
        total_by_type
            .entry(DataType::BlockEntity)
            .or_insert_with(T::new_empty);
        total_by_type
            .entry(DataType::Entity)
            .or_insert_with(T::new_empty);
        total_by_type
            .entry(DataType::Player)
            .or_insert_with(T::new_empty);

        Self {
            grouped,
            total_by_type,
            total_combined,
        }
    }

    /// Calculates the combined summary for a specific dimension.
    fn dimension_combined(&self, dimension: &str) -> T {
        let mut combined_summary = T::new_empty();
        if let Some(types_map) = self.grouped.get(dimension) {
            for item_summary in types_map.values() {
                combined_summary.merge(item_summary);
            }
        }
        combined_summary
    }
}

/// Trait to provide summary data in a generic way for different views.
pub trait SummaryDataProvider {
    type ItemSummary: Aggregable;

    fn get_grouped_data(&self) -> &BTreeMap<String, BTreeMap<DataType, Self::ItemSummary>>;
    fn get_total_block_entity_summary(&self) -> &Self::ItemSummary;
    fn get_total_entity_summary(&self) -> &Self::ItemSummary;
    fn get_total_player_data_summary(&self) -> &Self::ItemSummary;
    fn get_total_combined_summary(&self) -> &Self::ItemSummary;
    fn calculate_dimension_combined_summary(&self, dimension: &str) -> Self::ItemSummary;
}

impl<T: Aggregable> SummaryDataProvider for AggregationResult<T> {
    type ItemSummary = T;

    fn get_grouped_data(&self) -> &BTreeMap<String, BTreeMap<DataType, Self::ItemSummary>> {
        &self.grouped
    }

    fn get_total_block_entity_summary(&self) -> &Self::ItemSummary {
        self.total_by_type
            .get(&DataType::BlockEntity)
            .expect("BlockEntity total should always be present due to initialization in new()")
    }

    fn get_total_entity_summary(&self) -> &Self::ItemSummary {
        self.total_by_type
            .get(&DataType::Entity)
            .expect("Entity total should always be present due to initialization in new()")
    }

    fn get_total_player_data_summary(&self) -> &Self::ItemSummary {
        self.total_by_type
            .get(&DataType::Player)
            .expect("Player total should always be present due to initialization in new()")
    }

    fn get_total_combined_summary(&self) -> &Self::ItemSummary {
        &self.total_combined
    }
    fn calculate_dimension_combined_summary(&self, dimension: &str) -> Self::ItemSummary {
        self.dimension_combined(dimension)
    }
}
