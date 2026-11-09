use crate::db::schema::init_schema;

#[test]
fn test_db_init_and_migrations() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    let version: i32 = conn.query_row("SELECT version FROM schema_versions WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(version, 5);
}
