//! The hub edge: reads asks, answers, withdrawals and findings from the hub's SQLite database, read-only. Promise: it never writes to hub.db.

use crate::error::Error;
use crate::report::{AndonReport, FirstPassYield, Spread};
use rusqlite::{Connection, OpenFlags, params};
use std::path::Path;

pub fn read(path: &Path, days: u32, now_ms: i64) -> Result<AndonReport, Error> {
    let fail = |e: rusqlite::Error| Error::HubDb { path: path.to_path_buf(), why: e.to_string() };
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(fail)?;
    let since = now_ms - i64::from(days) * 86_400_000;

    let pairs = |sql: &str| -> Result<Vec<(String, i64)>, Error> {
        let mut st = db.prepare(sql).map_err(fail)?;
        let rows = st
            .query_map(params![since], |r| Ok((r.get::<_, String>("k")?, r.get::<_, i64>("n")?)))
            .map_err(fail)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(fail)
    };
    let asks_by_shape = pairs("SELECT shape AS k, count(*) AS n FROM asks WHERE opened_ms > ?1 GROUP BY shape ORDER BY n DESC")?;
    let findings_by_finder = pairs("SELECT found_by AS k, count(*) AS n FROM findings WHERE created_ms > ?1 GROUP BY found_by ORDER BY n DESC")?;

    let mut st = db
        .prepare(
            "SELECT a.shape AS shape, an.action AS action, count(*) AS n FROM asks a \
             JOIN answers an ON an.ask_id = a.id AND an.undone_ms IS NULL \
             WHERE a.opened_ms > ?1 GROUP BY a.shape, an.action ORDER BY a.shape, n DESC",
        )
        .map_err(fail)?;
    let answers_by_shape_and_action = st
        .query_map(params![since], |r| {
            Ok((r.get::<_, String>("shape")?, r.get::<_, String>("action")?, r.get::<_, i64>("n")?))
        })
        .map_err(fail)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(fail)?;

    let mut st = db
        .prepare(
            "SELECT (an.at_ms - a.opened_ms) / 60000.0 AS minutes FROM asks a \
             JOIN answers an ON an.ask_id = a.id AND an.undone_ms IS NULL WHERE a.opened_ms > ?1",
        )
        .map_err(fail)?;
    let minutes = st
        .query_map(params![since], |r| r.get::<_, f64>("minutes"))
        .map_err(fail)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(fail)?;

    let count = |sql: &str| db.query_row(sql, params![since], |r| r.get::<_, i64>("n")).map_err(fail);
    let withdrawn = count("SELECT count(*) AS n FROM withdrawals w JOIN asks a ON a.id = w.ask_id WHERE a.opened_ms > ?1")?;
    let open = count(
        "SELECT count(*) AS n FROM asks a WHERE a.opened_ms > ?1 \
         AND NOT EXISTS (SELECT 1 FROM answers an WHERE an.ask_id = a.id AND an.undone_ms IS NULL) \
         AND NOT EXISTS (SELECT 1 FROM withdrawals w WHERE w.ask_id = a.id)",
    )?;

    let first_pass = db
        .query_row(
            "WITH done AS (SELECT DISTINCT ticket_id FROM transitions WHERE status = 'done' AND at_ms > ?1) \
             SELECT count(*) AS reviewed, \
                    coalesce(sum(NOT EXISTS (SELECT 1 FROM findings f WHERE f.ticket_id = d.ticket_id AND f.kind = 'bug')), 0) AS clean \
             FROM done d WHERE EXISTS (SELECT 1 FROM runs r WHERE r.ticket_id = d.ticket_id AND r.role = 'review')",
            params![since],
            |r| Ok((r.get::<_, i64>("reviewed")?, r.get::<_, i64>("clean")?)),
        )
        .map_err(fail)?;
    let first_pass_yield = match first_pass {
        (0, _) => FirstPassYield::Unmeasured,
        (reviewed, clean) => FirstPassYield::Measured { reviewed, clean },
    };

    Ok(AndonReport {
        first_pass_yield,
        days,
        asks_by_shape,
        answers_by_shape_and_action,
        answer_minutes: Spread::of(minutes),
        withdrawn,
        open,
        findings_by_finder,
    })
}
