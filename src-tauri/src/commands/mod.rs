pub mod models;
pub mod collections;
pub mod tags;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedModels {
    pub models: Vec<ModelItem>,
    pub total: u32,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelFile {
    pub id: String,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub file_size: u64,
    pub file_path: String,
    pub is_primary_3d: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelItem {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub description: String,
    pub thumbnail_path: Option<String>,
    pub thumbnail_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub owner_username: String,
    pub is_favorited: bool,
    pub files_count: u32,
    pub tags: Vec<String>,
    pub primary_file: Option<ModelFile>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelDetail {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub description: String,
    pub thumbnail_path: Option<String>,
    pub thumbnail_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub owner_username: String,
    pub is_favorited: bool,
    pub tags: Vec<String>,
    pub files: Vec<ModelFile>,
    pub collections: Vec<CollectionRef>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub parent_folder_id: Option<String>,
    pub created_at: String,
    pub model_count: u32,
    pub children: Option<Vec<CollectionItem>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagItem {
    pub id: String,
    pub name: String,
    pub count: u32,
}