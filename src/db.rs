use std::{marker::PhantomData, path::PathBuf};

use sqlx::{sqlite::SqliteConnectOptions, Error};
use sqlx::{Connection, Sqlite, Transaction};
use sqlx::{Executor, SqliteConnection};

trait IntoExecutor<'c, E: Executor<'c, Database = Sqlite>> {
    fn to_executor(&mut self) -> E;
}

pub struct DbFactory {
    conn: SqliteConnection,
}

impl DbFactory {
    pub async fn connect() -> Result<DbFactory, Error> {
        let mut conn = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&"test.db")
                .create_if_missing(true),
        )
        .await?;

        sqlx::migrate!("./migrations").run(&mut conn).await?;

        Ok(DbFactory { conn })
    }

    pub fn db<'c>(self: &'c mut Self) -> Db<'c, &'c mut SqliteConnection> {
        Db {
            executor: &&mut self.conn,
            _marker: PhantomData::default(),
        }
    }

    pub async fn begin_transaction<'c>(self: &'c mut Self) -> Result<DbTransaction<'c>, Error> {
        Ok(DbTransaction {
            tx: self.conn.begin().await?,
        })
    }
}

struct DbTransaction<'c> {
    tx: Transaction<'c, Sqlite>,
}

impl<'c> DbTransaction<'c> {
    pub fn db(&'c mut self) -> Db<'c, &'c mut Transaction<'c, Sqlite>> {
        let tx: &'c mut Transaction<'c, Sqlite> = &mut self.tx;

        Db {
            executor: &tx,
            _marker: PhantomData::default(),
        }
    }
}

pub struct Db<'c, E>
where
    E: Executor<'c>,
{
    executor: E,
    _marker: PhantomData<&'c ()>,
}

#[derive(Clone, Debug)]
pub struct HashedFile {
    path: PathBuf,
    hash: String,
}

impl HashedFile {
    pub fn new(path: PathBuf, hash: String) -> HashedFile {
        HashedFile { path, hash }
    }
}

pub struct MappingInfo {
    pub pdf_hashes: Vec<String>,
    pub finished: bool,
}

impl<'c, E> Db<'c, E>
where
    E: Executor<'c, Database = Sqlite>,
{
    pub async fn update_hashes(
        &mut self,
        files: impl Iterator<Item = HashedFile>,
    ) -> Result<(), Error> {
        for file in files {
            let path: &str = &file.path.to_string_lossy();
            let hash = &file.hash;

            sqlx::query!(
                "
                    DELETE FROM files
                    WHERE file_path = ? OR hash = ?
                ",
                path,
                hash
            )
            .execute(self.executor)
            .await?;

            sqlx::query!(
                "
                    INSERT INTO files(file_path, hash)
                    VALUES (?, ?)
                ",
                path,
                hash
            )
            .execute(self.executor)
            .await?;
        }
        Ok(())
    }

    pub async fn find_mapping_info<'a>(
        &'a mut self,
        video: &HashedFile,
    ) -> Result<Option<MappingInfo>, Error> {
        let hash = &video.hash;

        let results = sqlx::query!(
            "
                SELECT videos.id as id, finished, videos_pdfs.pdf_hash as pdf_hash FROM videos
                LEFT JOIN videos_pdfs ON videos_pdfs.video_id = videos.id
                WHERE video_hash = ?
            ",
            hash
        )
        .fetch_all(self.executor)
        .await?;

        if results.len() == 0 {
            return Ok(None);
        }

        let finished = results[0].finished;
        let pdf_hashes = results
            .into_iter()
            .filter_map(|c| c.pdf_hash)
            .collect::<Vec<_>>();

        Ok(Some(MappingInfo {
            finished,
            pdf_hashes,
        }))
    }
}
