use super::*;

impl ProbeLedger {
    pub(crate) fn list_history(&self, query: &ProbeHistoryQuery) -> Result<ProbeHistoryPage, String> {
        let limit = query.limit.clamp(1, 50);
        let (where_clause, bindings) = history_filter(query);
        let Some(connection) = self.open_read()? else {
            return Ok(ProbeHistoryPage { items: Vec::new(), total: 0, offset: query.offset, limit });
        };
        let total: u32 = {
            let refs: Vec<&dyn rusqlite::ToSql> = bindings.iter().map(|value| value as &dyn rusqlite::ToSql).collect();
            connection
                .query_row(
                    &format!("SELECT count(*) FROM probe_batches b {where_clause}"),
                    rusqlite::params_from_iter(refs.iter()),
                    |row| row.get::<_, i64>(0).map(|value| value.max(0) as u32),
                )
                .map_err(db_error)?
        };
        let mut statement = connection
            .prepare(&format!(
                "SELECT b.id, b.started_at, b.finished_at, b.status, b.question_label, \
                 b.profile_id, b.profile_name, \
                 count(r.id), \
                 sum(CASE WHEN r.status='passed' THEN 1 ELSE 0 END), \
                 sum(CASE WHEN r.status IN ('passed','failed') THEN 1 ELSE 0 END), \
                 sum(CASE WHEN r.total_tokens IS NOT NULL THEN 1 ELSE 0 END), \
                 sum(r.total_tokens), b.grading_version \
                 FROM probe_batches b LEFT JOIN probe_runs r ON r.batch_id = b.id \
                 {where_clause} \
                 GROUP BY b.id ORDER BY b.started_at DESC, b.rowid DESC LIMIT ?{} OFFSET ?{}",
                bindings.len() + 1,
                bindings.len() + 2
            ))
            .map_err(db_error)?;
        let limit_value = limit as i64;
        let offset_value = query.offset as i64;
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = bindings.iter().map(|value| value as &dyn rusqlite::ToSql).collect();
        params_vec.push(&limit_value);
        params_vec.push(&offset_value);
        let rows = statement
            .query_map(params_vec.as_slice(), |row| {
                Ok(ProbeHistoryItem {
                    batch_id: row.get(0)?,
                    started_at: row.get(1)?,
                    finished_at: row.get(2)?,
                    status: ProbeBatchStatus::parse(&row.get::<_, String>(3)?)
                        .unwrap_or(ProbeBatchStatus::Failed),
                    question_label: row.get(4)?,
                    grading_version: map_enum(row, 12, crate::codex_probe::ProbeGrading::parse, "判分规则")?,
                    profile_id: row.get(5)?,
                    profile_name: row.get(6)?,
                    run_count: row.get::<_, i64>(7)?.max(0) as u32,
                    passed_count: row.get::<_, Option<i64>>(8)?.unwrap_or(0).max(0) as u32,
                    judged_count: row.get::<_, Option<i64>>(9)?.unwrap_or(0).max(0) as u32,
                    recorded_runs: row.get::<_, Option<i64>>(10)?.unwrap_or(0).max(0) as u32,
                    total_tokens: row
                        .get::<_, Option<i64>>(11)?
                        .map(|value| value.max(0) as u64),
                })
            })
            .map_err(db_error)?;
        let items = rows.collect::<Result<Vec<_>, _>>().map_err(db_error)?;
        Ok(ProbeHistoryPage { items, total, offset: query.offset, limit })
    }

    pub(crate) fn list_history_profiles(&self) -> Result<Vec<ProbeHistoryProfile>, String> {
        let Some(connection) = self.open_read()? else {
            return Ok(Vec::new());
        };
        let mut statement = connection
            .prepare(
                "SELECT b.profile_id, b.profile_name FROM probe_batches b \
                 WHERE b.profile_id IS NOT NULL AND b.rowid=(SELECT latest.rowid FROM probe_batches latest \
                 WHERE latest.profile_id=b.profile_id ORDER BY latest.started_at DESC, latest.rowid DESC LIMIT 1) \
                 ORDER BY b.profile_name, b.profile_id",
            )
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ProbeHistoryProfile {
                    profile_id: row.get(0)?,
                    profile_name: row.get(1)?,
                })
            })
            .map_err(db_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
    }

    // ------------------------------------------------------------ deletes

    /// Deletes finished batches and their runs. A running batch blocks the
    /// whole request; deleting only the radar's own records never touches
    /// Codex sessions or the metering ledger.
    pub(crate) fn delete_batches(&self, batch_ids: &[String]) -> Result<crate::storage::ReclaimOutcome, String> {
        if batch_ids.is_empty() {
            return Err("未选择要删除的检测记录".to_string());
        }
        let mut connection = self.open_write()?;
        let transaction = connection.transaction().map_err(db_error)?;
        let placeholders = std::iter::repeat("?")
            .take(batch_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let running: Option<String> = transaction
            .query_row(
                &format!(
                    "SELECT id FROM probe_batches WHERE id IN ({placeholders}) \
                     AND status='running' LIMIT 1"
                ),
                rusqlite::params_from_iter(batch_ids.iter()),
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(db_error(other)),
            })?;
        if let Some(id) = running {
            return Err(format!("检测 {id} 仍在进行，不能删除运行中的批次"));
        }
        transaction
            .execute(
                &format!("DELETE FROM probe_runs WHERE batch_id IN ({placeholders})"),
                rusqlite::params_from_iter(batch_ids.iter()),
            )
            .map_err(db_error)?;
        let deleted = transaction
            .execute(
                &format!("DELETE FROM probe_batches WHERE id IN ({placeholders})"),
                rusqlite::params_from_iter(batch_ids.iter()),
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
        Ok(crate::storage::ReclaimOutcome { removed: deleted as u64, warning: crate::storage::sqlite::compact(&connection).err() })
    }

}

fn history_filter(query: &ProbeHistoryQuery) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut bindings: Vec<String> = Vec::new();
    if let Some(days) = query.days.filter(|days| *days > 0) {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        conditions.push(format!(
            "b.started_at >= '{}'",
            cutoff.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        ));
    }
    match &query.profile {
        ProbeProfileFilter::All => {}
        ProbeProfileFilter::Unlinked => conditions.push("b.profile_id IS NULL".to_string()),
        ProbeProfileFilter::Id(id) => {
            bindings.push(id.clone());
            conditions.push(format!("b.profile_id = ?{}", bindings.len()));
        }
    }
    if let Some(status) = query.status {
        bindings.push(status.as_str().to_string());
        conditions.push(format!("b.status = ?{}", bindings.len()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    (where_clause, bindings)
}
