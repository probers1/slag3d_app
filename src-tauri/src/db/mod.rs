pub mod schema;

pub use schema::init_schema;

#[cfg(test)]
mod tests;

/// Simple DbState wrapper for future use
pub struct DbState {
    pub conn: rusqlite::Connection,
}

impl DbState {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }
}

// Placeholder query helpers - minimal implementations to satisfy compilation
pub fn list_models(_conn: &rusqlite::Connection) -> rusqlite::Result<Vec<String>> {
    Ok(vec![])
}

pub fn get_model_detail(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<Option<()>> {
    Ok(None)
}

pub fn insert_model(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn update_model(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn delete_model(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn list_collections(_conn: &rusqlite::Connection) -> rusqlite::Result<Vec<String>> {
    Ok(vec![])
}

pub fn create_collection(_conn: &rusqlite::Connection, _name: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn delete_collection(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn list_tags(_conn: &rusqlite::Connection) -> rusqlite::Result<Vec<String>> {
    Ok(vec![])
}
