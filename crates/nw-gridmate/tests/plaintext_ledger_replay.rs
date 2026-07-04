use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use nw_gridmate::replay::{ReplaySummary, replay_ledger_file};

#[test]
fn replays_plaintext_ledgers_from_env() {
    let ledgers = ledger_inputs();
    if ledgers.is_empty() {
        eprintln!(
            "skipping plaintext ledger replay; set NW_GRIDMATE_REPLAY_LEDGER or \
             NW_GRIDMATE_REPLAY_LEDGER_DIR"
        );
        return;
    }

    let strict = env_flag("NW_GRIDMATE_REPLAY_STRICT");
    let mut total = ReplaySummary::default();
    for ledger in ledgers {
        let stats =
            replay_ledger_file(&ledger).unwrap_or_else(|err| panic!("{}: {err}", ledger.display()));
        eprintln!(
            "{}: records={} datagrams={} carrier_messages={} channels={:?} hub={} state_bundles={} state_fragments={} decode_errors={}",
            ledger.display(),
            stats.records,
            stats.datagrams,
            stats.carrier_messages,
            stats.channels,
            stats.hub_messages,
            stats.state_bundles,
            stats.state_fragments,
            stats.total_parse_errors()
        );
        total.merge(stats);
    }

    assert!(total.records > 0, "expected at least one ledger record");
    assert!(
        total.datagrams > 0,
        "expected at least one carrier datagram"
    );
    assert!(
        total.carrier_messages > 0,
        "expected at least one reassembled carrier message"
    );

    if strict {
        assert_eq!(
            total.hub_parse_errors, 0,
            "Hub envelope parse errors: {:?}",
            total.hub_errors
        );
        assert_eq!(
            total.state_bundle_parse_errors, 0,
            "state bundle parse errors: {:?}",
            total.state_bundle_errors
        );
        assert_eq!(
            total.state_fragment_iter_errors, 0,
            "state fragment iterator errors: {:?}",
            total.state_fragment_errors
        );
        assert_eq!(
            total.state_fragment_decode_errors, 0,
            "state fragment decode errors: {:?}",
            total.state_fragment_errors
        );
    }
}

fn ledger_inputs() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["NW_GRIDMATE_REPLAY_LEDGER", "NW_GRIDMATE_REPLAY_LEDGER_DIR"] {
        if let Ok(value) = env::var(var) {
            roots.extend(env::split_paths(&value));
        }
    }

    let mut ledgers = Vec::new();
    for root in roots {
        collect_ledgers(&root, &mut ledgers)
            .unwrap_or_else(|err| panic!("collect ledgers from {}: {err}", root.display()));
    }
    ledgers.sort();
    ledgers.dedup();
    ledgers
}

fn collect_ledgers(path: &Path, ledgers: &mut Vec<PathBuf>) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        ledgers.push(path.to_path_buf());
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_ledgers(&child, ledgers)?;
        } else if is_ledger_file(&child) {
            ledgers.push(child);
        }
    }

    Ok(())
}

fn is_ledger_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "ledger.bin")
        || path.extension().is_some_and(|ext| ext == "nwdl")
}

fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}
