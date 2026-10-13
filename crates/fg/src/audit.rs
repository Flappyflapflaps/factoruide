//! `fg audit`: look for each known kind of data problem everywhere in an edition's data.

use anyhow::Result;
use fg_route::audit;
use fg_route::job::Overrides;
use rusqlite::Connection;
use std::path::Path;

/// Print the findings, the accepted ones apart; fails when errors are left (not accepted).
pub fn run(conn: &Connection, overrides_dir: &Path, checks: &[String], all: bool) -> Result<()> {
    let edition = crate::edition();
    for c in checks {
        anyhow::ensure!(
            audit::CHECKS.iter().any(|check| check.name == c),
            "unknown check {c} ({})",
            audit::CHECKS
                .iter()
                .map(|check| check.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let overrides = Overrides::load_edition(overrides_dir, edition)?;
    let accepted = audit::accepted(overrides_dir, edition)?;
    let findings = audit::audit(conn, &overrides, checks, &|m| eprintln!("{m}"))?;
    let mut open = 0;
    for check in audit::CHECKS {
        let name = &check.name;
        if !checks.is_empty() && !checks.iter().any(|c| c == name) {
            continue;
        }
        let of: Vec<_> = findings.iter().filter(|f| f.check == *name).collect();
        let reason = |key: &str| {
            accepted
                .iter()
                .find(|a| a.check == *name && a.key == key)
                .map(|a| a.reason.as_str())
        };
        let left = of.iter().filter(|f| reason(&f.key).is_none()).count();
        let verdict = if check.error {
            open += left;
            "to fix"
        } else {
            "to review"
        };
        println!(
            "== {name}: {left} {verdict}, {} accepted\n   {}",
            of.len() - left,
            check.what
        );
        for f in &of {
            match reason(&f.key) {
                None => println!(
                    "  [{}] {}\n      {} (races: {})",
                    name,
                    f.key,
                    f.detail,
                    f.races.join(", ")
                ),
                Some(r) if all => println!("  accepted [{}] {}: {r}", name, f.key),
                Some(_) => {}
            }
        }
    }
    anyhow::ensure!(open == 0, "{open} findings to fix ({})", edition.key());
    Ok(())
}
