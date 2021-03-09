/*
use rusqlite::{params, Connection, Result};
use schemamama::migration;
use schemamama_rusqlite::SqliteMigration;

pub struct CreateUsers;

migration!(CreateUsers, 1, "create users table");

impl SqliteMigration for CreateUsers {
    fn up(&self, conn: &Connection) -> Result<()> {
        conn.execute("CREATE TABLE users (id BIGINT PRIMARY KEY);", params![])?;
        Ok(())
    }

    fn down(&self, conn: &Connection) -> Result<()> {
        conn.execute("DROP TABLE users;", params![])?;
        Ok(())
    }
}


    let conn = Connection::open_in_memory().unwrap();
    let adapter = SqliteAdapter::new(Rc::new(RefCell::new(conn)));

    // Create the metadata tables necessary for tracking migrations. This is safe to call more than
    // once (`CREATE TABLE IF NOT EXISTS schemamama` is used internally):
    adapter.setup_schema();

    let mut migrator = Migrator::new(adapter);

    migrator.register(Box::new(CreateUsers));

    migrator.up(None).unwrap();

schemamama = "0.3.0"
schemamama_rusqlite = "0.8.0"

[dependencies.rusqlite]
version = "0.24.2"
features = ["bundled"]
*/
