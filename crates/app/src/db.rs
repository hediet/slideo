use anyhow::Result;
use app_dirs::{get_app_dir, AppDataType, AppInfo};
use serde::{Deserialize, Serialize};
use sqlx::{
    pool::PoolConnection,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Connection, Error, Pool, Sqlite, SqliteConnection, Transaction,
};
use std::fs::create_dir_all;
use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
};

use crate::pdf_to_images::PdfPage;
use matching::Matching;

#[derive(Clone)]
pub struct DbPool {
    pool: Pool<Sqlite>,
}

const APP_INFO: AppInfo = AppInfo {
    name: "Slideo",
    author: "hediet",
};

impl DbPool {
    pub async fn connect() -> Result<DbPool> {
        let path = get_app_dir(AppDataType::UserConfig, &APP_INFO, "db")?;
        create_dir_all(&path)?;

        let pool = SqlitePoolOptions::new()
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path.join("slideo.db"))
                    .create_if_missing(true),
            )
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(DbPool { pool })
    }

    pub async fn db(&self) -> Result<Db<'static>> {
        let r = self.pool.acquire().await?;
        Ok(Db {
            db: DbImpl::Conn(r),
            _marker: PhantomData::default(),
        })
    }
}

enum DbImpl<'a> {
    Conn(PoolConnection<Sqlite>),
    Trans(Transaction<'a, Sqlite>),
}

pub enum TransactionMarker {}

pub struct Db<'a, T = ()> {
    db: DbImpl<'a>,
    _marker: PhantomData<T>,
}

pub struct MappingInfo {
    pub pdf_hashes: Vec<String>,
    pub finished: bool,
}

impl<'a> Db<'a, TransactionMarker> {
    pub async fn commit(self) -> Result<(), Error> {
        match self.db {
            DbImpl::Conn(_conn) => panic!("Should not happen"),
            DbImpl::Trans(trans) => trans.commit().await?,
        }
        Ok(())
    }

    pub async fn set_pdf_extracted_pages_dir(
        &mut self,
        data: &PdfExtractedPagesDir,
    ) -> Result<(), Error> {
        sqlx::query!(
            "DELETE FROM pdf_extracted_pages_dirs WHERE pdf_hash = ?",
            data.pdf_hash
        )
        .execute(self.get_conn_mut())
        .await?;

        let dir = data.dir.to_string_lossy();
        let dir = &dir as &str;
        sqlx::query!(
            "INSERT INTO pdf_extracted_pages_dirs(pdf_hash, dir, finished) VALUES (?, ?, ?)",
            data.pdf_hash,
            dir,
            data.finished,
        )
        .execute(self.get_conn_mut())
        .await?;

        Ok(())
    }

    pub async fn update_hashes<'c>(
        &mut self,
        file_hashes: impl Iterator<Item = (&Path, &str)>,
    ) -> Result<(), Error> {
        for (path, hash) in file_hashes {
            let path = path.to_string_lossy();
            let path = path.as_ref();
            sqlx::query!(
                "DELETE FROM files WHERE file_path = ? OR hash = ?",
                path,
                hash
            )
            .execute(self.get_conn_mut())
            .await?;

            sqlx::query!(
                "INSERT INTO files(file_path, hash) VALUES (?, ?)",
                path,
                hash
            )
            .execute(self.get_conn_mut())
            .await?;
        }
        Ok(())
    }

    pub async fn create_or_reset_video(
        &mut self,
        video_hash: &str,
        pdf_hashs: impl Iterator<Item = &str>,
    ) -> Result<(), Error> {
        sqlx::query!("DELETE FROM videos WHERE video_hash = ?", video_hash)
            .execute(self.get_conn_mut())
            .await?;

        let row_id = sqlx::query!(
            "INSERT INTO videos(video_hash, finished) VALUES (?, false)",
            video_hash
        )
        .execute(self.get_conn_mut())
        .await?
        .last_insert_rowid();

        for pdf_hash in pdf_hashs {
            sqlx::query!(
                "INSERT INTO videos_pdfs(video_id, pdf_hash) VALUES (?, ?)",
                row_id,
                pdf_hash
            )
            .execute(self.get_conn_mut())
            .await?;
        }

        Ok(())
    }

    pub async fn update_video_matchings<'c>(
        &mut self,
        video_hash: &str,
        matchings: impl Iterator<Item = &'c Matching<&'c PdfPage<'c>>>,
    ) -> Result<(), Error> {
        let video_id = sqlx::query!("SELECT id FROM videos WHERE video_hash = ?", video_hash)
            .fetch_one(self.get_conn_mut())
            .await?
            .id;

        sqlx::query!("UPDATE videos SET finished = true WHERE id = ?", video_id)
            .execute(self.get_conn_mut())
            .await?;

        for matching in matchings {
            let pdf_hash = matching.image.map(|p| p.pdf_hash);
            let video_ms = matching.video_time.as_millis() as u32;
            let page_offset = matching.image.map(|p| (p.page_nr - 1) as u32).unwrap_or(0);
            sqlx::query!(
                "INSERT INTO videos_mapping(video_id, video_ms, pdf_hash, page) VALUES (?, ?, ?, ?)",
                video_id,
                video_ms,
                pdf_hash,
                page_offset,
            )
            .execute(self.get_conn_mut())
            .await?;
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
pub struct PdfVideoMatching {
    video_offset_ms: u32,
    pdf_hash: String,
    video_hash: String,
    page_idx: u32,
    duration_ms: u32,
}

impl<'a, T> Db<'a, T> {
    pub async fn begin_trans<'c>(&'c mut self) -> Result<Db<'c, TransactionMarker>, Error> {
        let trans = self.get_conn_mut().begin().await?;
        Ok(Db {
            db: DbImpl::Trans(trans),
            _marker: PhantomData::default(),
        })
    }

    pub async fn get_pdf_video_matchings(
        &mut self,
        pdf_hash: &str,
    ) -> Result<Vec<PdfVideoMatching>> {
        let video_ids = sqlx::query!(
            "SELECT DISTINCT video_id FROM videos_pdfs WHERE pdf_hash = ?",
            pdf_hash
        )
        .fetch_all(self.get_conn_mut())
        .await?;

        let mut result: Vec<PdfVideoMatching> = Vec::new();

        // fetch all mappings of all those videos
        for video_id in video_ids {
            let mappings = sqlx::query!(
                "
                    SELECT video_ms, pdf_hash, page, video_hash FROM videos_mapping
                    INNER JOIN videos ON videos.id = video_id
                    WHERE video_id = ?
                    ORDER BY video_ms ASC
                ",
                video_id.video_id
            )
            .fetch_all(self.get_conn_mut())
            .await?;

            let mut iter = mappings.into_iter().peekable();
            loop {
                match iter.next() {
                    Some(mapping) => {
                        let next = iter.peek();
                        let duration_ms = if let Some(next) = next {
                            next.video_ms - mapping.video_ms
                        } else {
                            5000 // should not happen anymore
                        };

                        match mapping.pdf_hash {
                            Some(mapping_pdf_hash) if mapping_pdf_hash == pdf_hash => {
                                result.push(PdfVideoMatching {
                                    duration_ms: duration_ms as u32,
                                    video_offset_ms: mapping.video_ms as u32,
                                    pdf_hash: mapping_pdf_hash,
                                    video_hash: mapping.video_hash,
                                    page_idx: mapping.page.unwrap_or(0) as u32,
                                });
                            }
                            _ => {}
                        }
                    }
                    None => {
                        break;
                    }
                }
            }
        }

        Ok(result)
    }

    pub async fn get_path(&mut self, hash: &str) -> Result<Option<PathBuf>> {
        let result = sqlx::query!(
            "
                SELECT file_path FROM files
                WHERE hash = ?
            ",
            hash
        )
        .fetch_optional(self.get_conn_mut())
        .await?;

        Ok(result.map(|r| r.file_path.into()))
    }

    pub async fn find_mapping_info(
        &mut self,
        video_hash: &str,
    ) -> Result<Option<MappingInfo>, Error> {
        let results = sqlx::query!(
            "
                SELECT videos.id as id, finished, videos_pdfs.pdf_hash as pdf_hash FROM videos
                LEFT JOIN videos_pdfs ON videos_pdfs.video_id = videos.id
                WHERE video_hash = ?
            ",
            video_hash
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

    pub async fn get_pdf_extracted_pages_dir(
        &mut self,
        pdf_hash: &str,
    ) -> Result<Option<PdfExtractedPagesDir>, Error> {
        let results = sqlx::query!(
            "
                SELECT pdf_hash, dir, finished FROM pdf_extracted_pages_dirs
                WHERE pdf_hash = ?
            ",
            pdf_hash
        )
        .fetch_all(self.get_conn_mut())
        .await?;

        if let Some(record) = results.first() {
            Ok(Some(PdfExtractedPagesDir {
                pdf_hash: record.pdf_hash.clone(),
                dir: record.dir.clone().into(),
                finished: record.finished,
            }))
        } else {
            Ok(None)
        }
    }

    fn get_conn_mut(&mut self) -> &mut SqliteConnection {
        match &mut self.db {
            DbImpl::Conn(conn) => conn,
            DbImpl::Trans(trans) => trans,
        }
    }
}

pub struct PdfExtractedPagesDir {
    pub pdf_hash: String,
    pub dir: PathBuf,
    pub finished: bool,
}
