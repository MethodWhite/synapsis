//! Regression tests for the FTS index staying in sync with `observations`.
//!
//! Background: `observations_fts` is a plain `fts5(title, content)` table, not an
//! external-content one. The previous code pruned entries with
//!
//! ```sql
//! INSERT INTO observations_fts(observations_fts, rowid, ...) VALUES('delete', ?1, '', '')
//! ```
//!
//! which is the *external-content* delete command. Against a plain fts5 table it
//! always fails, and the failure was discarded with `let _ =`, so the index was
//! never pruned. Soft deletes only bloated the index (search filters on
//! `o.deleted_at IS NULL`, so those rows stayed invisible), but **updates and
//! revisions left the previous text behind**: a search for wording that had been
//! edited out still matched, returning rows whose current content no longer
//! contained the query.
//!
//! These tests pin that behaviour so a `let _ =` cannot hide a regression again.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, Once, OnceLock};

use synapsis::domain::{Observation, ObservationType, SessionId};
use synapsis::infrastructure::database::{Database, migration};

/// One isolated database per test binary, behind a Mutex.
///
/// Two things matter here. `Database::new()` falls back to
/// `dirs::data_local_dir()/synapsis`, the LIVE coordination database, so the
/// data dir must be redirected or these tests write into real shared memory.
/// And the tests share one database, so access is serialised: `run_migrations`
/// has no concurrency guard of its own and concurrent `init` calls race with
/// "duplicate column name: prev_hash".
fn db() -> MutexGuard<'static, Database> {
    static CELL: OnceLock<Mutex<Database>> = OnceLock::new();
    static ENV: Once = Once::new();
    let cell = CELL.get_or_init(|| {
        ENV.call_once(|| {
            let dir =
                std::env::temp_dir().join(format!("synapsis-test-fts-{}", std::process::id()));
            std::fs::create_dir_all(&dir).ok();
            // SAFETY: test-scoped env change, applied once before any Database::new().
            unsafe {
                std::env::set_var("SYNAPSIS_DATA_DIR", &dir);
            }
        });
        let database = Database::new();
        // Database::new() only opens the connection; the schema comes from here.
        migration::run_migrations(&database.get_conn()).expect("migrate test database");
        Mutex::new(database)
    });
    cell.lock().unwrap_or_else(|e| e.into_inner())
}

fn marker() -> String {
    static N: AtomicU32 = AtomicU32::new(0);
    format!("zondaible{}", N.fetch_add(1, Ordering::SeqCst))
}

fn save(db: &Database, title: &str, content: String) -> i64 {
    db.save_observation_direct(&Observation::new(
        SessionId::new("test"),
        ObservationType::Manual,
        title.to_string(),
        content,
    ))
    .ok()
    .expect("save observation")
    .0
}

/// How many rows in the FTS index still match `needle`.
fn fts_rows(db: &Database, needle: &str) -> i64 {
    db.get_conn()
        .query_row(
            "SELECT count(*) FROM observations_fts WHERE observations_fts MATCH ?1",
            [needle],
            |r| r.get(0),
        )
        .unwrap_or(-1)
}

#[test]
fn update_removes_previous_text_from_search_index() {
    let db = db();
    let m = marker();

    let id = save(&db, "Titulo inicial", format!("contenido original {m}"));
    assert_eq!(
        fts_rows(&db, &m),
        1,
        "la observacion nueva debe estar indexada"
    );

    // Rewrite the content WITHOUT the marker.
    db.update_observation(id, "Titulo final", "contenido totalmente distinto")
        .expect("update observation");

    assert_eq!(
        fts_rows(&db, &m),
        0,
        "tras actualizar, el texto anterior debe salir del indice. Si sigue \
         apareciendo, el FTS conservo la version vieja y las busquedas \
         devuelven filas cuyo contenido actual ya no coincide con lo pedido"
    );
}

#[test]
fn soft_delete_removes_row_from_search_index() {
    let db = db();
    let m = marker();

    let id = save(
        &db,
        "Titulo descartable",
        format!("contenido descartable {m}"),
    );
    assert_eq!(fts_rows(&db, &m), 1);

    db.soft_delete_observation(id).expect("soft delete");

    // Search already hid these via `o.deleted_at IS NULL`, so assert on the index
    // itself: the row must be gone, not merely hidden.
    assert_eq!(
        fts_rows(&db, &m),
        0,
        "la fila borrada debe salir del indice FTS, no solo ocultarse"
    );
}
