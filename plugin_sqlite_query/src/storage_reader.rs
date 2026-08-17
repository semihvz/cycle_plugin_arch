use rusqlite::{Connection, Result, types::ValueRef};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub execution_time_ms: f64,
    pub formatted_output: String,
}

pub struct StorageReader {
    conn: Arc<Mutex<Connection>>,
    pub db_path: String,
}

impl StorageReader {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        // Enable WAL mode & query_only optimization
        let _ = conn.execute_batch("
            PRAGMA journal_mode = WAL;
            PRAGMA read_uncommitted = ON;
        ");
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: db_path.to_string(),
        })
    }

    pub fn execute_sql(&self, sql: &str) -> Result<QueryResult> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(sql)?;
        let col_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();
        let col_count = col_names.len();

        let mut rows_data: Vec<Vec<String>> = Vec::new();
        let mut rows_iter = stmt.query([])?;

        while let Some(row) = rows_iter.next()? {
            let mut row_vec = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let val_ref = row.get_ref(i)?;
                let val_str = match val_ref {
                    ValueRef::Null => "NULL".to_string(),
                    ValueRef::Integer(i) => i.to_string(),
                    ValueRef::Real(f) => format!("{:.6}", f).trim_end_matches('0').trim_end_matches('.').to_string(),
                    ValueRef::Text(b) => String::from_utf8_lossy(b).into_owned(),
                    ValueRef::Blob(b) => format!("<BLOB {}b>", b.len()),
                };
                row_vec.push(val_str);
            }
            rows_data.push(row_vec);
        }

        let duration = start.elapsed().as_secs_f64() * 1000.0;
        let formatted = format_ascii_table(&col_names, &rows_data, duration);

        Ok(QueryResult {
            columns: col_names,
            row_count: rows_data.len(),
            rows: rows_data,
            execution_time_ms: duration,
            formatted_output: formatted,
        })
    }

    pub fn list_tables(&self) -> Result<QueryResult> {
        self.execute_sql("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
    }

    pub fn get_schema(&self, table_name: &str) -> Result<QueryResult> {
        // Sanitize table_name against dangerous chars
        let clean_name = table_name.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
        self.execute_sql(&format!("PRAGMA table_info({})", clean_name))
    }

    pub fn get_db_file_size(&self) -> u64 {
        std::fs::metadata(&self.db_path).map(|m| m.len()).unwrap_or(0)
    }
}

pub fn format_ascii_table(cols: &[String], rows: &[Vec<String>], duration_ms: f64) -> String {
    if cols.is_empty() {
        return format!("(0 rows returned, {:.2} ms)\n", duration_ms);
    }

    let mut col_widths: Vec<usize> = cols.iter().map(|c| c.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < col_widths.len() {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }
    }

    let mut out = String::new();
    
    // Header separator line
    out.push('+');
    for &w in &col_widths {
        out.push_str(&"-".repeat(w + 2));
        out.push('+');
    }
    out.push('\n');

    // Header column names
    out.push('|');
    for (i, c) in cols.iter().enumerate() {
        let w = col_widths[i];
        out.push_str(&format!(" {:<width$} |", c, width = w));
    }
    out.push('\n');

    // Header separator line
    out.push('+');
    for &w in &col_widths {
        out.push_str(&"-".repeat(w + 2));
        out.push('+');
    }
    out.push('\n');

    // Data rows
    for row in rows {
        out.push('|');
        for (i, cell) in row.iter().enumerate() {
            let w = col_widths.get(i).copied().unwrap_or(cell.len());
            out.push_str(&format!(" {:<width$} |", cell, width = w));
        }
        out.push('\n');
    }

    // Bottom separator line
    out.push('+');
    for &w in &col_widths {
        out.push_str(&"-".repeat(w + 2));
        out.push('+');
    }
    out.push('\n');

    out.push_str(&format!("({} rows returned in {:.2} ms)\n", rows.len(), duration_ms));
    out
}
