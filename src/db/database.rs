use std::cell::RefCell;
use std::rc::Rc;

use diesel::connection::SimpleConnection;
use diesel::{Connection, SqliteConnection};
use tracing::{info, warn};

use crate::config;

use super::init::init_db;

/// Shared, single-process handle to the app's SQLite connection.
///
/// GTK runs on one thread, so an `Rc<RefCell<…>>` is the right shape — cheap to
/// clone into every component that needs the database, no pool required.
/// Borrowing is scoped to a single synchronous `with` call, which never overlaps
/// because Relm4 components process messages one at a time.
#[derive(Clone)]
pub struct Database(Rc<RefCell<SqliteConnection>>);

impl Database {
    /// Open the app database at the configured path: create the data directory,
    /// run the foreign-schema guard, and apply pending migrations. Returns
    /// `None` (with a logged warning) if any step fails, mirroring the previous
    /// `init_database` behavior.
    pub fn open() -> Option<Self> {
        let db_path = config::db_path();

        if let Some(parent) = db_path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            warn!("Failed to create database directory: {e}");
            return None;
        }

        let path = db_path.to_string_lossy();
        match SqliteConnection::establish(&path) {
            Ok(mut conn) => {
                // Tune the connection before migrating: WAL for better
                // read/write concurrency, and a busy timeout so a transient
                // lock waits rather than erroring immediately. (diesel already
                // enables `foreign_keys = ON` per connection.)
                if let Err(e) =
                    conn.batch_execute("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
                {
                    warn!("Failed to apply SQLite pragmas: {e}");
                }
                if let Err(e) = init_db(&mut conn) {
                    warn!("Failed to initialize database: {e}");
                    return None;
                }
                info!("Database initialized at {}", db_path.display());
                Some(Self(Rc::new(RefCell::new(conn))))
            }
            Err(e) => {
                warn!("Failed to open database: {e}");
                None
            }
        }
    }

    /// Run `f` with mutable access to the connection. The borrow is released
    /// when `f` returns.
    pub fn with<R>(&self, f: impl FnOnce(&mut SqliteConnection) -> R) -> R {
        f(&mut self.0.borrow_mut())
    }
}
