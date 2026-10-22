use std::sync::Mutex;

use sidex_db::Database;

pub struct SidexDbState {
    db: Mutex<Database>,
}

impl SidexDbState {
    pub fn new(db: Database) -> Self {
        Self { db: Mutex::new(db) }
    }

    /// Borrow the database directly. Used by chat-session storage,
    /// which needs a guard rather than a single-shot query helper.
    pub fn lock_db(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, Database>,
        std::sync::PoisonError<std::sync::MutexGuard<'_, Database>>,
    > {
        self.db.lock()
    }
}
