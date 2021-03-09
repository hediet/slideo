use std::{marker::PhantomData, path::PathBuf};

use sqlx::{sqlite::SqliteConnectOptions, Database, Error};
use sqlx::{Connection, Sqlite, Transaction};
use sqlx::{Executor, SqliteConnection};

enum DbImpl<'a> {
    Conn(SqliteConnection),
    Trans(Transaction<'a, Sqlite>),
}

pub enum TransactionMarker {}

pub struct Db<'a, T> {
    db: DbImpl<'a>,
    _marker: PhantomData<T>,
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

impl<'a> Db<'a, TransactionMarker> {
    pub async fn commit(self) -> Result<(), Error> {
        match self.db {
            DbImpl::Conn(conn) => panic!("Should not happen"),
            DbImpl::Trans(conn) => conn.commit().await?,
        }
        Ok(())
    }

    pub async fn rollback(self) -> Result<(), Error> {
        match self.db {
            DbImpl::Conn(conn) => panic!("Should not happen"),
            DbImpl::Trans(conn) => conn.rollback().await?,
        }
        Ok(())
    }
}

impl Db<'static, ()> {
    pub async fn connect() -> Result<Db<'static, ()>, Error> {
        let mut conn = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&"test.db")
                .create_if_missing(true),
        )
        .await?;

        sqlx::migrate!("./migrations").run(&mut conn).await?;

        Ok(Db {
            db: DbImpl::Conn(conn),
            _marker: PhantomData::default(),
        })
    }
}

impl<'a, T> Db<'a, T> {
    pub async fn begin_trans<'c>(&'c mut self) -> Result<Db<'c, TransactionMarker>, Error> {
        let trans = self.get_conn_mut().begin().await?;
        Ok(Db {
            db: DbImpl::Trans(trans),
            _marker: PhantomData::default(),
        })
    }

    fn get_conn_mut(&mut self) -> &mut SqliteConnection {
        match &mut self.db {
            DbImpl::Conn(conn) => conn,
            DbImpl::Trans(conn) => conn,
        }
    }

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
            .execute(self.get_conn_mut())
            .await?;

            sqlx::query!(
                "
                    INSERT INTO files(file_path, hash)
                    VALUES (?, ?)
                ",
                path,
                hash
            )
            .execute(self.get_conn_mut())
            .await?;
        }
        Ok(())
    }

    pub async fn find_mapping_info(
        &mut self,
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
        .fetch_all(self.get_conn_mut())
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
