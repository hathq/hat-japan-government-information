use hat_specifications::{HatActionStatus, HatInvocation};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::thread;
use std::time::{Duration, Instant};

use crate::worker_io::{
    arguments, canonical_directory, common_args, message, path, required, run_hatter,
    write_document,
};
use crate::worker_task::process;

#[derive(Deserialize)]
pub(super) struct Lease {
    pub(super) record: LeaseRecord,
}

#[derive(Deserialize)]
pub(super) struct LeaseRecord {
    pub(super) status: HatActionStatus,
    pub(super) invocation: HatInvocation,
}

#[derive(Deserialize)]
struct RegistrationLease {
    worker_id: String,
    revision: u64,
}

pub(super) fn run() -> Result<(), String> {
    let args = arguments()?;
    let registration = serde_json::json!({
        "worker_id": required(&args, "worker-id")?,
        "worker_identity_ref": required(&args, "worker-identity-ref")?,
        "placement_selection_digest_sha256": required(&args, "placement-digest")?,
        "expected_revision": 0,
        "ttl_seconds": 15
    });
    let control = canonical_directory(required(&args, "state-dir")?)?;
    let registration_path = control.join("registration-cas.json");
    write_document(
        &registration_path,
        &serde_json::to_vec(&registration).map_err(message)?,
    )?;
    let common = common_args(&args, hat_japan_government_information::REPOSITORY_ID)?;
    let response = run_hatter(
        &args,
        "worker-register",
        &common,
        &["--registration-json", path(&registration_path)?],
    )?;
    finish_registered(&args, &common, &control, &response)
}

fn finish_registered(
    args: &BTreeMap<String, String>,
    common: &[String],
    control: &std::path::Path,
    response: &[u8],
) -> Result<(), String> {
    let registration: RegistrationLease = serde_json::from_slice(response).map_err(message)?;
    if registration.worker_id != required(args, "worker-id")? || registration.revision == 0 {
        return Err("worker registration response differs from the requested owner".into());
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        if let Some(lease) = wait_for_claim(args, common)? {
            process(args, common, control, &lease)
        } else {
            println!("{{\"processed\":false}}");
            Ok(())
        }
    }))
    .map_err(|_| "worker task panicked".to_owned())
    .and_then(std::convert::identity);
    let revision = registration.revision.to_string();
    let unregister = run_hatter(
        args,
        "worker-unregister",
        common,
        &[
            "--worker-id",
            required(args, "worker-id")?,
            "--expected-revision",
            &revision,
        ],
    )
    .map(|_| ());
    match (result, unregister) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}; worker unregister failed: {cleanup}")),
    }
}

fn wait_for_claim(
    args: &BTreeMap<String, String>,
    common: &[String],
) -> Result<Option<Lease>, String> {
    let seconds = args
        .get("wait-seconds")
        .map_or(Ok(10), |value| value.parse::<u64>())
        .map_err(message)?;
    if seconds > 30 {
        return Err("--wait-seconds exceeds 30".into());
    }
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        let claim = run_hatter(
            args,
            "claim",
            common,
            &["--worker-id", required(args, "worker-id")?],
        )?;
        if let Some(lease) = serde_json::from_slice::<Option<Lease>>(&claim).map_err(message)? {
            return Ok(Some(lease));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(100));
    }
}
