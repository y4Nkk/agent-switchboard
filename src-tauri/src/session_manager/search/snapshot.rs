use rusqlite::{params, Connection, Statement};

use super::{SessionIssue, SessionMeta, SessionProject, SessionSearchHit, SessionSearchPage, PAGE_SIZE};

pub(in crate::session_manager) struct SearchSnapshot {
    connection: Connection,
    total: usize,
    pub(super) issues: Vec<SessionIssue>,
    projects: Vec<SessionProject>,
    tags: Vec<String>,
}

pub(in crate::session_manager) struct SearchRows<'db> {
    sessions: Statement<'db>,
    hits: Statement<'db>,
    total: usize,
}

impl SearchRows<'_> {
    pub(super) fn insert_session(&mut self, session: &SessionMeta) -> Result<i64, String> {
        self.sessions.insert([serde_json::to_string(session).map_err(error)?]).map_err(error)
    }

    pub(super) fn insert_hit(&mut self, session: i64, message: Option<&str>, offset: usize, excerpt: &str)
        -> Result<(), String> {
        self.hits.execute(params![session, message, offset, excerpt]).map_err(error)?;
        self.total += 1;
        Ok(())
    }
}

impl SearchSnapshot {
    pub(in crate::session_manager) fn build(projects: Vec<SessionProject>, tags: Vec<String>,
        fill: impl FnOnce(&mut SearchRows<'_>) -> Result<Vec<SessionIssue>, String>,
    ) -> Result<Self, String> {
        // SQLite owns the unnamed temporary database for exactly this connection's lifetime.
        let mut connection = Connection::open("").map_err(error)?;
        let transaction = connection.transaction().map_err(error)?;
        transaction.execute_batch("CREATE TABLE sessions (id INTEGER PRIMARY KEY, body TEXT NOT NULL);
            CREATE TABLE hits (id INTEGER PRIMARY KEY, session INTEGER NOT NULL, message TEXT,
                content_offset INTEGER NOT NULL, excerpt TEXT NOT NULL);").map_err(error)?;
        let (total, issues) = {
            let mut rows = SearchRows {
                sessions: transaction.prepare("INSERT INTO sessions (body) VALUES (?)").map_err(error)?,
                hits: transaction.prepare("INSERT INTO hits (session, message, content_offset, excerpt)
                    VALUES (?, ?, ?, ?)").map_err(error)?,
                total: 0,
            };
            let issues = fill(&mut rows)?;
            (rows.total, issues)
        };
        transaction.commit().map_err(error)?;
        Ok(Self { connection, total, issues, projects, tags })
    }

    pub(in crate::session_manager) fn page(&self, page: usize) -> Result<SessionSearchPage, String> {
        let page = page.min(self.total.saturating_sub(1) / PAGE_SIZE + 1);
        let offset = (page - 1) * PAGE_SIZE;
        let mut results = Vec::with_capacity(PAGE_SIZE);
        if offset < self.total {
            let mut query = self.connection.prepare_cached("SELECT sessions.body, hits.message, hits.content_offset, hits.excerpt
                FROM hits JOIN sessions ON sessions.id = hits.session WHERE hits.id > ? ORDER BY hits.id LIMIT ?")
                .map_err(error)?;
            let mut rows = query.query(params![offset, PAGE_SIZE]).map_err(error)?;
            while let Some(row) = rows.next().map_err(error)? {
                let body: String = row.get(0).map_err(error)?;
                results.push(SessionSearchHit { session: serde_json::from_str(&body).map_err(error)?,
                    message_id: row.get(1).map_err(error)?, content_offset: row.get(2).map_err(error)?,
                    excerpt: row.get(3).map_err(error)? });
            }
        }
        Ok(SessionSearchPage { results, total: self.total, page, page_size: PAGE_SIZE, issues: self.issues.clone(),
            projects: self.projects.clone(), tags: self.tags.clone() })
    }
}

fn error(error: impl std::fmt::Display) -> String {
    format!("会话搜索结果快照不可用：{error}")
}
