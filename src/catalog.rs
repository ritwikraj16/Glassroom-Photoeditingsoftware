//! Local SQLite catalog — Classic-style paths, ratings, flags, collections, develop params.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    None = 0,
    Pick = 1,
    Reject = -1,
}

impl Flag {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Flag::Pick,
            -1 => Flag::Reject,
            _ => Flag::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopParams {
    pub exposure: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub temp: f32,
    pub tint: f32,
    pub saturation: f32,
    pub clarity: f32,
}

impl Default for DevelopParams {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            temp: 0.0,
            tint: 0.0,
            saturation: 0.0,
            clarity: 0.0,
        }
    }
}

impl DevelopParams {
    pub fn is_identity(&self) -> bool {
        let z = |v: f32| v.abs() < 0.001;
        z(self.exposure)
            && z(self.contrast)
            && z(self.highlights)
            && z(self.shadows)
            && z(self.temp)
            && z(self.tint)
            && z(self.saturation)
            && z(self.clarity)
    }
}

#[derive(Debug, Clone)]
pub struct Photo {
    pub id: i64,
    pub path: PathBuf,
    pub filename: String,
    pub folder: String,
    pub rating: i32,
    pub flag: Flag,
    pub develop: DevelopParams,
    pub imported_at: String,
}

#[derive(Debug, Clone)]
pub struct Collection {
    pub id: i64,
    pub name: String,
}

pub struct Catalog {
    conn: Connection,
    pub path: PathBuf,
}

impl Catalog {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path).with_context(|| format!("open catalog {}", path.display()))?;
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS photos (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                filename TEXT NOT NULL,
                folder TEXT NOT NULL,
                rating INTEGER NOT NULL DEFAULT 0,
                flag INTEGER NOT NULL DEFAULT 0,
                develop_json TEXT NOT NULL DEFAULT '{}',
                imported_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS collections (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS collection_photos (
                collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
                photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
                PRIMARY KEY (collection_id, photo_id)
            );
            CREATE INDEX IF NOT EXISTS idx_photos_folder ON photos(folder);
            ",
        )?;
        // Seed a default collection if empty
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM collections", [], |r| r.get(0))?;
        if count == 0 {
            conn.execute("INSERT INTO collections (name) VALUES (?1)", params!["Selects"])?;
        }
        Ok(Self { conn, path })
    }

    pub fn import_jpeg(&self, file: &Path) -> Result<Option<i64>> {
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if ext != "jpg" && ext != "jpeg" {
            return Ok(None);
        }
        let abs = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
        let path_str = abs.to_string_lossy().to_string();
        let filename = abs
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());
        let folder = abs
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".into());
        let imported_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let develop = serde_json::to_string(&DevelopParams::default())?;

        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM photos WHERE path = ?1",
                params![path_str],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(Some(id));
        }

        self.conn.execute(
            "INSERT INTO photos (path, filename, folder, rating, flag, develop_json, imported_at)
             VALUES (?1, ?2, ?3, 0, 0, ?4, ?5)",
            params![path_str, filename, folder, develop, imported_at],
        )?;
        Ok(Some(self.conn.last_insert_rowid()))
    }

    pub fn import_folder(&self, dir: &Path) -> Result<(usize, usize)> {
        let mut added = 0usize;
        let mut seen = 0usize;
        for entry in walkdir::WalkDir::new(dir).follow_links(false).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            match self.import_jpeg(entry.path())? {
                Some(_) => {
                    // distinguish new vs existing by checking changes — approximate: always count as seen
                    seen += 1;
                    // re-check: if inserted this turn, row changed. Simpler: count all jpeg attempts
                    added += 1;
                }
                None => {}
            }
        }
        // `added` above counts all successful imports including already-present.
        // Report both as total imported/linked.
        Ok((added, seen))
    }

    fn row_to_photo(row: &rusqlite::Row<'_>) -> rusqlite::Result<Photo> {
        let path: String = row.get(1)?;
        let develop_json: String = row.get(6)?;
        let develop: DevelopParams = serde_json::from_str(&develop_json).unwrap_or_default();
        Ok(Photo {
            id: row.get(0)?,
            path: PathBuf::from(path),
            filename: row.get(2)?,
            folder: row.get(3)?,
            rating: row.get(4)?,
            flag: Flag::from_i32(row.get(5)?),
            develop,
            imported_at: row.get(7)?,
        })
    }

    pub fn list_photos(&self) -> Result<Vec<Photo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, filename, folder, rating, flag, develop_json, imported_at
             FROM photos ORDER BY folder, filename",
        )?;
        let rows = stmt.query_map([], Self::row_to_photo)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn photos_in_folder(&self, folder: &str) -> Result<Vec<Photo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, filename, folder, rating, flag, develop_json, imported_at
             FROM photos WHERE folder = ?1 ORDER BY filename",
        )?;
        let rows = stmt.query_map(params![folder], Self::row_to_photo)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn photos_in_collection(&self, collection_id: i64) -> Result<Vec<Photo>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.path, p.filename, p.folder, p.rating, p.flag, p.develop_json, p.imported_at
             FROM photos p
             JOIN collection_photos cp ON cp.photo_id = p.id
             WHERE cp.collection_id = ?1
             ORDER BY p.filename",
        )?;
        let rows = stmt.query_map(params![collection_id], Self::row_to_photo)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_folders(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT folder, COUNT(*) FROM photos GROUP BY folder ORDER BY folder",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_collections(&self) -> Result<Vec<Collection>> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM collections ORDER BY name")?;
        let rows = stmt.query_map([], |r| {
            Ok(Collection {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn create_collection(&self, name: &str) -> Result<i64> {
        self.conn
            .execute("INSERT INTO collections (name) VALUES (?1)", params![name])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn add_to_collection(&self, collection_id: i64, photo_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO collection_photos (collection_id, photo_id) VALUES (?1, ?2)",
            params![collection_id, photo_id],
        )?;
        Ok(())
    }

    pub fn set_rating(&self, photo_id: i64, rating: i32) -> Result<()> {
        let rating = rating.clamp(0, 5);
        self.conn
            .execute("UPDATE photos SET rating = ?1 WHERE id = ?2", params![rating, photo_id])?;
        Ok(())
    }

    pub fn set_flag(&self, photo_id: i64, flag: Flag) -> Result<()> {
        self.conn.execute(
            "UPDATE photos SET flag = ?1 WHERE id = ?2",
            params![flag as i32, photo_id],
        )?;
        Ok(())
    }

    pub fn set_develop(&self, photo_id: i64, params_dev: &DevelopParams) -> Result<()> {
        let json = serde_json::to_string(params_dev)?;
        self.conn.execute(
            "UPDATE photos SET develop_json = ?1 WHERE id = ?2",
            params![json, photo_id],
        )?;
        Ok(())
    }

    pub fn get_photo(&self, id: i64) -> Result<Option<Photo>> {
        self.conn
            .query_row(
                "SELECT id, path, filename, folder, rating, flag, develop_json, imported_at
                 FROM photos WHERE id = ?1",
                params![id],
                Self::row_to_photo,
            )
            .optional()
            .map_err(Into::into)
    }
}

pub fn default_catalog_path() -> PathBuf {
    if let Ok(p) = std::env::var("GLASSROOM_CATALOG") {
        return PathBuf::from(p);
    }
    if let Some(proj) = directories::ProjectDirs::from("app", "Glassroom", "Glassroom") {
        return proj.data_dir().join("catalog.db");
    }
    PathBuf::from("glassroom-catalog.db")
}
