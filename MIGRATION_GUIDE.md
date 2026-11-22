# Database schema & migrations

SLAG3D keeps a single SQLite database, `vault.db`, in the OS application-data directory. The current schema version is **v6**.

## How migrations run

`init_schema()` in `src-tauri/src/db/schema.rs` runs on every startup, before any command can access the DB:

1. Pragmas are applied (`journal_mode=WAL`, `foreign_keys=ON`).
2. `schema_versions` and all tables/indexes are created with `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`.
3. The single local user (`00000000-0000-0000-0000-000000000000`) is inserted if missing.
4. Legacy soft-delete columns are removed from `models`, `files`, `collections` and `users`.
5. `schema_versions` is upserted to version 6.

Migrations are idempotent: running them against an already-current vault is a no-op.

## Version history

| Version | Change |
|---------|--------|
| v1–v4 | From the retired Express server: base tables, metadata columns, soft-delete flags for users |
| v5 | Tauri conversion: simplified schema for a single local user, no auth tables |
| **v6** | Hard deletes: `is_deleted` / `deleted_at` dropped from all tables; legacy v4/v5 vaults are upgraded on open |

There is no soft-delete path in the desktop app — `delete_model` removes the metadata and the files on disk.

## Opening an older vault

Copy an older `vault.db` into the app data directory (or restore an older backup) and start the app. `init_schema()` upgrades it in place. Nothing needs to be edited by hand.

`import_backup` reads the schema version and model count from the backed-up **database** and rejects an archive whose `manifest.json` disagrees with them. A backup made by a newer schema is rejected instead of being partially restored; update the app first.

## Changing the schema

Edit `src-tauri/src/db/schema.rs`:

- Add idempotent `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE` / `DROP COLUMN` statements to `init_schema()`.
- Bump the version written in the final `INSERT ... ON CONFLICT DO UPDATE` statement.
- Use `drop_soft_delete_columns`-style guards for columns that may not exist on older vaults.
- Add a test in `src-tauri/src/db/` for the upgrade path and run `cargo test`.

## Verification

```bash
cd src-tauri && cargo test
sqlite3 <app-data>/vault.db "SELECT * FROM schema_versions;"
# expected: 1|6|<timestamp>
```
