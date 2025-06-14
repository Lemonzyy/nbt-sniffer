use super::structures::{ReportItemDetailed, ReportItemId, ReportItemNbt};
use crate::{counter::Counter, escape_nbt_string};
use std::collections::HashMap;

pub fn to_detailed_item_entries(counter: &Counter) -> Vec<ReportItemDetailed> {
    let mut detailed_vec: Vec<_> = counter
        .detailed_counts()
        .iter()
        .map(|(item_key, &count)| (item_key.id.clone(), item_key.components_snbt.clone(), count))
        .collect();

    detailed_vec.sort_by(|(a_id, a_nbt, a_count), (b_id, b_nbt, b_count)| {
        b_count
            .cmp(a_count)
            .then_with(|| a_id.cmp(b_id))
            .then_with(|| a_nbt.cmp(b_nbt))
    });

    detailed_vec
        .iter()
        .map(|(id, nbt_opt, count)| ReportItemDetailed {
            count: *count,
            id: id.clone(),
            nbt: nbt_opt.as_ref().map(|s| escape_nbt_string(s)),
        })
        .collect()
}

pub fn to_id_item_entries(map: &HashMap<String, u64>) -> Vec<ReportItemId> {
    let mut vec: Vec<_> = map.iter().map(|(id, &count)| (id.clone(), count)).collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    vec.iter()
        .map(|(id, count)| ReportItemId {
            count: *count,
            id: id.clone(),
        })
        .collect()
}

pub fn to_nbt_item_entries(counter: &Counter) -> Vec<ReportItemNbt> {
    let mut by_nbt_vec: Vec<_> = counter.total_by_nbt().into_iter().collect();
    by_nbt_vec.sort_by(|(a_nbt, a_count), (b_nbt, b_count)| {
        b_count.cmp(a_count).then_with(|| a_nbt.cmp(b_nbt))
    });

    by_nbt_vec
        .iter()
        .map(|(nbt_opt, count)| ReportItemNbt {
            count: *count,
            nbt: nbt_opt.as_ref().map(|s| escape_nbt_string(s)),
        })
        .collect()
}
