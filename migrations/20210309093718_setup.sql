CREATE TABLE pdf_extracted_pages_dirs (
    pdf_hash TEXT PRIMARY KEY,
    dir TEXT NOT NULL UNIQUE,
    finished BOOLEAN NOT NULL
);
CREATE TABLE files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT UNIQUE NOT NULL,
    hash TEXT UNIQUE NOT NULL
);
CREATE TABLE videos (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    video_hash TEXT NOT NULL UNIQUE,
    finished BOOLEAN NOT NULL
);
CREATE TABLE videos_pdfs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    video_id INTEGER REFERENCES videos (id) ON DELETE CASCADE,
    pdf_hash TEXT NOT NULL,
    UNIQUE (video_id, pdf_hash)
);
CREATE TABLE videos_mapping (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    video_id INTEGER REFERENCES videos (id) ON DELETE CASCADE,
    video_ms INTEGER NOT NULL,
    pdf_hash TEXT,
    page INTEGER,
    UNIQUE (video_id, video_ms)
);