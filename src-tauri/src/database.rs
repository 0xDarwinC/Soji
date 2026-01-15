use crate::models::Sticker;
use rusqlite::{Connection, Result, ToSql};
use std::path::Path;

pub fn init_db(path: &Path) -> Result<()> {
    let conn = Connection::open(path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS stickers (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            name TEXT NOT NULL,
            pack TEXT NOT NULL,
            format TEXT NOT NULL,
            thumbnail_path TEXT,
            width INTEGER,
            height INTEGER,
            is_favorite INTEGER DEFAULT 0,
            last_used INTEGER DEFAULT 0,
            use_count INTEGER DEFAULT 0
        )",
        [],
    )?;
    Ok(())
}

fn parse_sticker(row: &rusqlite::Row) -> Result<Sticker> {
    Ok(Sticker {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        thumbnail_path: row.get(3).unwrap_or(row.get(2)?),
        format: row.get(4)?,
        pack: row.get(5)?,
        is_favorite: row.get::<_, i32>(6)? == 1,
        width: row.get(7).unwrap_or(0),
        height: row.get(8).unwrap_or(0),
    })
}

pub fn search_stickers(conn: &Connection, query: String, tab: String, limit: usize) -> Result<Vec<Sticker>> {
    let mut stickers = Vec::new();
    let query_pattern = format!("%{}%", query);
    let limit_val = limit as i64;

    let sql = if tab == "Recents" {
        "SELECT id, name, path, thumbnail_path, format, pack, is_favorite, width, height FROM stickers WHERE use_count > 0 ORDER BY last_used DESC LIMIT ?1"
    } else if tab == "Favorites" {
        "SELECT id, name, path, thumbnail_path, format, pack, is_favorite, width, height FROM stickers WHERE is_favorite = 1 AND name LIKE ?2 LIMIT ?1"
    } else if tab == "All" {
        "SELECT id, name, path, thumbnail_path, format, pack, is_favorite, width, height FROM stickers WHERE name LIKE ?2 LIMIT ?1"
    } else {
        "SELECT id, name, path, thumbnail_path, format, pack, is_favorite, width, height FROM stickers WHERE pack = ?3 AND name LIKE ?2 LIMIT ?1"
    };

    let mut stmt = conn.prepare(sql)?;
    
    let rows = if tab == "Recents" {
        stmt.query_map(&[&limit_val as &dyn ToSql], parse_sticker)?
    } else if tab == "All" || tab == "Favorites" {
        stmt.query_map(&[&limit_val as &dyn ToSql, &query_pattern as &dyn ToSql], parse_sticker)?
    } else {
        stmt.query_map(&[&limit_val as &dyn ToSql, &query_pattern as &dyn ToSql, &tab as &dyn ToSql], parse_sticker)?
    };

    for sticker in rows {
        if let Ok(s) = sticker {
            stickers.push(s);
        }
    }

    Ok(stickers)
}

pub fn get_packs(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT pack FROM stickers ORDER BY pack")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    
    let mut packs = Vec::new();
    for pack in rows {
        if let Ok(p) = pack { packs.push(p); }
    }
    Ok(packs)
}

pub fn toggle_favorite(conn: &Connection, path: &str) -> Result<bool> {
    let is_fav: bool = conn.query_row(
        "SELECT is_favorite FROM stickers WHERE path = ?1", 
        [path], 
        |row| row.get(0)
    ).unwrap_or(false);

    let new_val = if is_fav { 0 } else { 1 };
    conn.execute("UPDATE stickers SET is_favorite = ?1 WHERE path = ?2", [&new_val as &dyn ToSql, &path as &dyn ToSql])?;
    
    Ok(!is_fav)
}

pub fn update_usage(conn: &Connection, path: &str, timestamp: i64) -> Result<()> {
    conn.execute(
        "UPDATE stickers SET use_count = use_count + 1, last_used = ?1 WHERE path = ?2",
        [&timestamp as &dyn ToSql, &path as &dyn ToSql],
    )?;
    Ok(())
}

pub fn wipe_history(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE stickers SET use_count = 0, last_used = 0", [])?;
    Ok(())
}

pub fn wipe_favorites(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE stickers SET is_favorite = 0", [])?;
    Ok(())
}

pub fn reset_library(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM stickers", [])?;
    conn.execute("VACUUM", [])?;
    Ok(())
}