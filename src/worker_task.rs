use hat_specifications::{
    ACTION_RESULT_SCHEMA, ActionReference, HatActionResult, HatInvocationOutcome,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::worker::Lease;
use crate::worker_io::{canonical_directory, message, path, required, run_hatter, write_document};

pub(super) fn process(
    args: &BTreeMap<String, String>,
    common: &[String],
    control: &Path,
    lease: &Lease,
) -> Result<(), String> {
    let invocation = &lease.record.invocation;
    if invocation.operation_id != hat_japan_government_information::OPERATION_ID
        || invocation.input.owner_id != "zixcel-graph"
    {
        return Err("invocation is outside the Japan government information contract".into());
    }
    let input_store = canonical_directory(required(args, "input-store")?)?;
    let input_bytes = read_digest_document(&input_store, &invocation.input)?;
    let input = serde_json::from_slice(&input_bytes).map_err(message)?;
    let output =
        hat_japan_government_information::prepare_request(input).map_err(ToOwned::to_owned)?;
    let bytes = serde_json::to_vec(&output).map_err(message)?;
    let digest = hex::encode(Sha256::digest(&bytes));
    let output_store = canonical_directory(required(args, "output-store")?)?;
    write_document(&output_store.join(format!("{digest}.json")), &bytes)?;
    complete(args, common, control, lease, &digest)?;
    println!("{{\"processed\":true,\"outputDigestSha256\":\"{digest}\"}}");
    Ok(())
}

fn complete(
    args: &BTreeMap<String, String>,
    common: &[String],
    control: &Path,
    lease: &Lease,
    digest: &str,
) -> Result<(), String> {
    let invocation = &lease.record.invocation;
    let result = HatActionResult {
        schema: ACTION_RESULT_SCHEMA.into(),
        invocation_id: invocation.invocation_id.clone(),
        operation_id: hat_japan_government_information::OPERATION_ID.into(),
        state_revision: lease.record.status.state_revision.saturating_add(1),
        projection_revision: invocation.expected_projection_revision.saturating_add(1),
        outcome: HatInvocationOutcome::Completed,
        output: Some(ActionReference {
            owner_id: hat_japan_government_information::REPOSITORY_ID.into(),
            reference: digest.into(),
            schema_id: hat_japan_government_information::OUTPUT_SCHEMA.into(),
            digest_sha256: digest.into(),
        }),
        reason_id: None,
        evidence_refs: Vec::new(),
    };
    let result_path = control.join(format!("result-{}.json", result.invocation_id));
    write_document(&result_path, &serde_json::to_vec(&result).map_err(message)?)?;
    run_hatter(
        args,
        "complete",
        common,
        &[
            "--worker-id",
            required(args, "worker-id")?,
            "--result-json",
            path(&result_path)?,
        ],
    )?;
    Ok(())
}

fn read_digest_document(root: &Path, reference: &ActionReference) -> Result<Vec<u8>, String> {
    if reference.schema_id != hat_japan_government_information::INPUT_SCHEMA
        || reference.reference != reference.digest_sha256
    {
        return Err("input reference is not content-addressed".into());
    }
    let bytes = fs::read(root.join(format!("{}.json", reference.reference))).map_err(message)?;
    if bytes.len() > 1_048_576 || hex::encode(Sha256::digest(&bytes)) != reference.digest_sha256 {
        return Err("input digest differs".into());
    }
    Ok(bytes)
}
