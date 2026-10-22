use super::*;

impl ProbeLedger {
    // ------------------------------------------------------------- writes

    pub(crate) fn insert_batch(&self, batch: &NewProbeBatch) -> Result<(), String> {
        let mut connection = self.open_write()?;
        let transaction = connection.transaction().map_err(db_error)?;
        transaction
            .execute(
                "INSERT INTO probe_batches (id, started_at, status, planned_runs, question_id, \
                 question_label, question_text, expected_answer, grading_version, cli_version, \
                 profile_id, profile_name, profile_model, reasoning_effort, connection_identity, \
                 config_fingerprint) \
                 VALUES (?1,?2,'running',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                params![
                    batch.id,
                    batch.started_at,
                    batch.planned_runs as i64,
                    batch.question.id,
                    batch.question.label,
                    batch.question.text,
                    batch.question.expected_answer,
                    batch.grading_version.as_str(),
                    batch.cli_version,
                    batch.config.profile_id,
                    batch.config.profile_name,
                    batch.config.profile_model,
                    batch.config.reasoning_effort,
                    batch.config.connection_identity,
                    batch.config.fingerprint
                ],
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)
    }

    pub(crate) fn start_run(&self, batch_id: &str, seq: u32) -> Result<(), String> {
        let mut connection = self.open_write()?;
        let transaction = connection.transaction().map_err(db_error)?;
        let changed = transaction
            .execute(
                "INSERT INTO probe_runs (batch_id, seq, status) SELECT ?1,?2,'running' \
                 WHERE EXISTS (SELECT 1 FROM probe_batches WHERE id=?1 AND status='running' AND planned_runs>=?2) \
                 AND NOT EXISTS (SELECT 1 FROM probe_runs WHERE batch_id=?1 AND status='running') \
                 AND ?2=(SELECT COALESCE(MAX(seq),0)+1 FROM probe_runs WHERE batch_id=?1)",
                params![batch_id, seq as i64],
            )
            .map_err(db_error)?;
        if changed != 1 { return Err("检测批次不可执行或上一次运行尚未结束".into()); }
        transaction.commit().map_err(db_error)
    }

    /// Persists the CLI-reported session id the moment it arrives, so an
    /// abrupt exit can still recover that run's usage.
    pub(crate) fn save_run_session(&self, batch_id: &str, seq: u32, session_id: &str) -> Result<(), String> {
        let mut connection = self.open_write()?;
        let transaction = connection.transaction().map_err(db_error)?;
        let changed = transaction
            .execute(
                "UPDATE probe_runs SET session_id=?3 WHERE batch_id=?1 AND seq=?2 AND status='running' AND (session_id IS NULL OR session_id=?3)",
                params![batch_id, seq as i64, session_id],
            )
            .map_err(db_error)?;
        if changed != 1 { return Err("检测会话记录不存在、已结束或会话标识冲突".into()); }
        transaction.commit().map_err(db_error)
    }

    /// Full row update for one finished (or spawn-failed) run. Idempotent so
    /// the save-retry path can replay it.
    pub(crate) fn save_run_result(
        &self,
        batch_id: &str,
        record: &ProbeRunRecord,
    ) -> Result<(), String> {
        let mut connection = self.open_write()?;
        let transaction = connection.transaction().map_err(db_error)?;
        write_run(&transaction, batch_id, record)?;
        transaction.commit().map_err(db_error)
    }

    /// Last results and terminal state commit together; failure leaves a recoverable batch.
    pub(crate) fn finish_batch(
        &self, batch_id: &str, status: ProbeBatchStatus,
        status_error: Option<String>, pending_runs: &[ProbeRunRecord],
    ) -> Result<(), String> {
        if !status.is_terminal() { return Err("结束批次需要终态".into()); }
        let mut connection = self.open_write()?;
        let transaction = connection.transaction().map_err(db_error)?;
        for record in pending_runs { write_run(&transaction, batch_id, record)?; }
        let running: i64 = transaction.query_row(
            "SELECT count(*) FROM probe_runs WHERE batch_id=?1 AND status='running'",
            [batch_id], |row| row.get(0),
        ).map_err(db_error)?;
        if running != 0 { return Err("仍有检测结果未结束，不能归档批次".into()); }
        let changed = transaction.execute(
            "UPDATE probe_batches SET status=?2, finished_at=?3, status_error=?4 WHERE id=?1 AND status='running'",
            params![batch_id, status.as_str(), now_rfc3339(), status_error],
        ).map_err(db_error)?;
        if changed != 1 { return Err("检测批次不存在或已经结束".into()); }
        transaction.commit().map_err(db_error)
    }
}

fn write_run(transaction: &rusqlite::Transaction<'_>, batch_id: &str, record: &ProbeRunRecord) -> Result<(), String> {
    if record.status == ProbeRunStatus::Running { return Err("结果必须具有终态".into()); }
    let changed = transaction
        .execute(
            "UPDATE probe_runs SET status=?3, session_id=COALESCE(?4, session_id), final_answer=?5, \
             reported_model=?6, duration_ms=?7, reasoning_tokens=?8, total_tokens=?9, \
             execution_error=?10, usage_error=?11 WHERE batch_id=?1 AND seq=?2 \
             AND (session_id IS NULL OR ?4 IS NULL OR session_id=?4) \
             AND EXISTS (SELECT 1 FROM probe_batches WHERE id=?1 AND status='running')",
            params![
                batch_id,
                record.seq as i64,
                record.status.as_str(),
                record.session_id,
                record.final_answer,
                record.reported_model,
                record.duration_ms.map(|value| value as i64),
                record.reasoning_tokens.map(|value| value as i64),
                record.total_tokens.map(|value| value as i64),
                record.execution_error,
                record.usage_error
            ],
        )
        .map_err(db_error)?;
    if changed != 1 { return Err("检测运行记录不存在，结果未保存".into()); }
    Ok(())
}
