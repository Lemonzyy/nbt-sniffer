use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
pub struct ReportItemDetailed {
    pub count: u64,
    pub id: String,
    pub nbt: String,
}

#[derive(Serialize, Clone)]
pub struct ReportItemId {
    pub count: u64,
    pub id: String,
}

#[derive(Serialize, Clone)]
pub struct ReportItemNbt {
    pub count: u64,
    pub nbt: String,
}

#[derive(Serialize)]
pub struct Report<TItem: Serialize> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_dimension: Option<HashMap<String, Vec<TItem>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_data_type: Option<HashMap<String, Vec<TItem>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_dimension_detail: Option<HashMap<String, HashMap<String, Vec<TItem>>>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub grand_total: Vec<TItem>,
    pub grand_total_count: u64,
}
