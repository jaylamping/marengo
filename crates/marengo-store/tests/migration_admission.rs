//! G15: refuse unversioned nonempty stores and connections without rollback journals.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::bounded;
use marengo_store::Store;
use rusqlite::Connection;

#[derive(Clone, Debug, PartialEq, Eq)]
struct View {
    schema: Vec<(String, String, String)>,
    settings: Vec<(String, String, i64)>,
    neighbor: Vec<(i64, String)>,
}
fn observe(conn: &Connection) -> View {
    let mut s = conn
        .prepare("SELECT type,name,COALESCE(sql,'') FROM sqlite_schema ORDER BY type,name")
        .expect("complete schema");
    let schema = s
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .expect("schema rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("schema values");
    let settings = if conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='settings' AND type='table')",
            [],
            |r| r.get::<_, bool>(0),
        )
        .expect("settings presence")
    {
        let mut s = conn
            .prepare("SELECT key,value_json,updated_ms FROM settings ORDER BY key")
            .expect("settings observation");
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .expect("settings rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("settings values")
    } else {
        Vec::new()
    };
    let mut s = conn
        .prepare("SELECT id,note FROM sqliteXneighbor ORDER BY id")
        .expect("literal neighbor observation");
    let neighbor = s
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("neighbor rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("neighbor values");
    View {
        schema,
        settings,
        neighbor,
    }
}
const NEIGHBOR: &str="CREATE TABLE sqliteXneighbor(id INTEGER PRIMARY KEY,note TEXT NOT NULL); INSERT INTO sqliteXneighbor VALUES(7,'independent opaque evidence');";

#[test]
fn unversioned_sqlitex_neighbor_is_refused_without_creating_store_schema() {
    bounded(
        "unversioned_sqlitex_neighbor_is_refused_without_creating_store_schema",
        || {
            let dir = tempfile::tempdir().expect("exclusive fixture");
            let root = dir.path().to_path_buf();
            assert!(root.is_absolute());
            let db = root.join("neighbor.sqlite");
            let conn = Connection::open(&db).expect("literal fixture connection");
            conn.execute_batch(NEIGHBOR)
                .expect("valid unversioned nonempty SQLite database");
            let before = observe(&conn);
            drop(conn);
            let result = Store::open(&db, &root);
            let refused = result.is_err();
            let error = result.as_ref().err().map(ToString::to_string);
            drop(result);
            let conn = Connection::open(&db).expect("independent after observation");
            let after = observe(&conn);
            drop(conn);
            dir.close().expect("actual fixture removal");
            let cleaned = !root.try_exists().expect("cleanup readback");
            let controls = cleaned
                && before.schema.len() == 1
                && before.schema[0].1 == "sqliteXneighbor"
                && before.settings.is_empty()
                && before.neighbor == vec![(7, "independent opaque evidence".into())];
            if controls {
                println!("G15_NONEMPTY_MARKER_CONTROLS_OK");
            }
            assert!(controls && refused && after==before,
          "unversioned nonempty store was admitted or modified\ncleanup={cleaned}; refused={refused}; error={error:?}; before={before:?}; after={after:?}");
        },
    );
}

#[test]
fn public_migrate_refuses_disabled_rollback_journal_before_any_logical_write() {
    bounded(
        "public_migrate_refuses_disabled_rollback_journal_before_any_logical_write",
        || {
            let dir = tempfile::tempdir().expect("exclusive fixture");
            let root = dir.path().to_path_buf();
            assert!(root.is_absolute());
            let db = root.join("journal.sqlite");
            let store = Store::open(&db, &root).expect("supported Store bootstrap control");
            let before = {
                let conn = store.connection();
                conn.execute_batch(NEIGHBOR)
                    .expect("literal independent neighbor");
                conn.execute_batch("UPDATE settings SET value_json='3',updated_ms=101 WHERE key='schema_version'; UPDATE settings SET value_json='7',updated_ms=23 WHERE key='log_archive_days'; UPDATE settings SET value_json='123456',updated_ms=29 WHERE key='log_disk_budget_bytes';").expect("explicit version and independent supplied settings");
                observe(&conn)
            };
            let prior_mode = {
                let conn = store.connection();
                conn.pragma_update(None, "journal_mode", "OFF")
                    .expect("actual caller disabled rollback journaling");
                conn.pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                    .expect("actual mode readback")
            };
            let result = store.migrate();
            let refused = result.is_err();
            let error = result.as_ref().err().map(ToString::to_string);
            let after = observe(&store.connection());
            let after_mode = store
                .connection()
                .pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                .expect("after mode readback");
            drop(store);
            dir.close().expect("actual fixture removal");
            let cleaned = !root.try_exists().expect("cleanup readback");
            let controls = cleaned
                && prior_mode == "off"
                && before.neighbor == vec![(7, "independent opaque evidence".into())]
                && before
                    .settings
                    .contains(&("schema_version".into(), "3".into(), 101))
                && before
                    .settings
                    .contains(&("log_archive_days".into(), "7".into(), 23))
                && before
                    .settings
                    .contains(&("log_disk_budget_bytes".into(), "123456".into(), 29));
            if controls {
                println!("G15_DISABLED_JOURNAL_CONTROLS_OK");
            }
            assert!(controls && refused && after==before && after_mode=="off",
          "disabled rollback journal was admitted or logically changed\ncleanup={cleaned}; refused={refused}; error={error:?}; before_mode={prior_mode}; after_mode={after_mode}; before={before:?}; after={after:?}");
        },
    );
}
