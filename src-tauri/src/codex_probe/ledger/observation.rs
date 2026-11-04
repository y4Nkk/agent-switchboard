use super::*;
use crate::codex_probe::observation::{ObservationSnapshot, RunObservation};

const COLUMNS: &str = "id, status, planned_runs, question_label, grading_version, status_error";

fn map_observation(row: &rusqlite::Row<'_>) -> rusqlite::Result<ObservationSnapshot> {
    Ok(ObservationSnapshot {
        batch_id: row.get(0)?, status: map_enum(row, 1, ProbeBatchStatus::parse, "批次状态")?,
        planned_runs: row.get(2)?, question_label: row.get(3)?,
        grading_version: map_enum(row, 4, crate::codex_probe::ProbeGrading::parse, "判分规则")?,
        status_error: row.get(5)?, runs: Vec::new(),
    })
}

impl ProbeLedger {
    pub(in crate::codex_probe) fn load_observation(&self, batch_id: Option<&str>) -> Result<Option<ObservationSnapshot>, String> {
        let Some(mut connection) = self.open_read()? else { return Ok(None); };
        let transaction = connection.transaction().map_err(db_error)?;
        let query = match batch_id {
            Some(_) => format!("SELECT {COLUMNS} FROM probe_batches WHERE id=?1"),
            None => format!("SELECT {COLUMNS} FROM probe_batches ORDER BY started_at DESC, rowid DESC LIMIT 1"),
        };
        let selected = match batch_id {
            Some(id) => transaction.query_row(&query, [id], map_observation),
            None => transaction.query_row(&query, [], map_observation),
        };
        let Some(mut snapshot) = selected.optional().map_err(db_error)? else { return Ok(None); };
        snapshot.runs = read_runs(&transaction, &snapshot.batch_id)?;
        transaction.commit().map_err(db_error)?;
        Ok(Some(snapshot))
    }
}

fn read_runs(connection: &Connection, batch_id: &str) -> Result<Vec<RunObservation>, String> {
    let mut statement = connection.prepare(
        "SELECT seq, status, execution_error IS NOT NULL FROM probe_runs WHERE batch_id=?1 ORDER BY seq",
    ).map_err(db_error)?;
    let rows = statement.query_map([batch_id], |row| Ok(RunObservation {
        seq: row.get(0)?, status: map_enum(row, 1, ProbeRunStatus::parse, "运行状态")?,
        execution_failed: row.get(2)?,
    })).map_err(db_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
}
