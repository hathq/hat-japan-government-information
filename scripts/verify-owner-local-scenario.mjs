// Added by HATHQ, 2026.
// Purpose: exercise one complete Hatter invocation through the released external HAT worker.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, realpath, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawn, spawnSync } from 'node:child_process'

const [hatter, worker, hatterHome, contextPartitionId] = process.argv.slice(2)
if (![hatter, worker, hatterHome].every(value => value?.startsWith('/'))
  || !/^partition-[0-9a-f]{64}$/u.test(contextPartitionId ?? '')) {
  throw new Error('usage: verify-owner-local-scenario HATTER WORKER HATTER_HOME CONTEXT_PARTITION_ID')
}
for (const executable of [hatter, worker]) {
  const entry = await stat(executable)
  if (!entry.isFile() || (entry.mode & 0o111) === 0 || await realpath(executable) !== executable) {
    throw new Error('scenario executable is not one exact executable')
  }
}
if (!(await stat(hatterHome)).isDirectory() || await realpath(hatterHome) !== hatterHome) {
  throw new Error('Hatter home is not one exact directory')
}

const repositoryId = 'hat-japan-government-information'
const operationId = 'hathq://vocabulary/action/prepare-government-information-request/v1'
const root = await mkdtemp(join(tmpdir(), 'hatter-japan-scenario-'))
const inputStore = join(root, 'input')
const outputStore = join(root, 'output')
const stateDir = join(root, 'worker')
await Promise.all([inputStore, outputStore, stateDir].map(path =>
  mkdir(path, { mode: 0o700 })))

try {
  const binding = commandJson('hat', 'binding', '--repository-id', repositoryId,
    '--context-partition-id', contextPartitionId)
  const placement = commandJson('hat', 'placement-read', '--repository-id', repositoryId,
    '--context-partition-id', contextPartitionId)
  const projection = commandJson('hat', 'projection-journal',
    '--context-partition-id', contextPartitionId,
    '--package-id', binding.package_id, '--from-revision', '0', '--limit', '256')
  const input = {
    schema: 'hathq://hat-japan-government-information/government-information-request-input/v1',
    jurisdiction: 'JP',
    topic: 'laws',
    question: 'Which official source contains the applicable Japanese law?',
  }
  const inputBytes = Buffer.from(JSON.stringify(input))
  const inputDigest = digest(inputBytes)
  await writeFile(join(inputStore, `${inputDigest}.json`), inputBytes,
    { mode: 0o600, flag: 'wx' })

  const workerId = 'japan-government-information-verification-worker'
  const child = spawn(worker, [
    '--hatter', hatter,
    '--hatter-home', hatterHome,
    '--context-partition-id', contextPartitionId,
    '--worker-id', workerId,
    '--worker-identity-ref', placement.location.identity_authority_ref,
    '--placement-digest', placement.selection_digest_sha256,
    '--input-store', inputStore,
    '--output-store', outputStore,
    '--state-dir', stateDir,
    '--wait-seconds', '10',
  ], { stdio: ['ignore', 'pipe', 'pipe'] })
  const childResult = collect(child)
  await waitForWorker(workerId)

  const invocationId = `japan-government-information-${Date.now()}`
  const invocation = {
    schema: 'hathq://hat/invocation/v4',
    invocation_id: invocationId,
    binding: {
      schema: 'hathq://hat/binding/v2',
      package_id: binding.package_id,
      package_digest_sha256: binding.package_sha256,
      catalog_digest_sha256: binding.catalog_digest_sha256,
      fitting_digest_sha256: binding.fitting_digest_sha256,
      subject_ref: binding.subject_ref,
      scope_ref: binding.scope_ref,
    },
    context_partition: {
      context_partition_id: binding.context_partition_id,
      revision: binding.revision,
      policy_digest_sha256: binding.policy_digest_sha256,
    },
    operation_id: operationId,
    expected_projection_revision: projection.to_revision,
    idempotency_key: invocationId,
    input: {
      owner_id: 'zixcel-graph',
      reference: inputDigest,
      schema_id: input.schema,
      digest_sha256: inputDigest,
    },
    effective_grant: {
      owner_id: 'hatter',
      reference: binding.subject_ref,
      schema_id: 'hathq://hat/effective-grant/v1',
      digest_sha256: binding.policy_digest_sha256,
    },
    placement: {
      owner_id: 'hatter',
      reference: placement.selection.selection_id,
      schema_id: 'hathq://hat/placement-selection/v2',
      digest_sha256: placement.selection_digest_sha256,
    },
  }
  const invocationPath = join(root, 'invocation.json')
  await writeFile(invocationPath, JSON.stringify(invocation), { mode: 0o600, flag: 'wx' })
  commandJson('hat', 'invoke', '--repository-id', repositoryId,
    '--invocation-json', invocationPath)
  const workerResult = await childResult
  if (workerResult.code !== 0) throw new Error(workerResult.stderr)
  const receipt = JSON.parse(workerResult.stdout.trim())
  const outputBytes = await readFile(join(outputStore, `${receipt.outputDigestSha256}.json`))
  const output = JSON.parse(outputBytes)
  if (digest(outputBytes) !== receipt.outputDigestSha256
    || output.schema !== 'hathq://hat-japan-government-information/government-information-request-output/v1'
    || output.status !== 'needs-user-input'
    || output.unresolved_fields?.join(',') !== 'as-of-date'
    || output.sources?.length !== 1
    || output.sources[0].origin !== 'https://laws.e-gov.go.jp'
    || !output.constraints?.includes('do-not-fill-unresolved')) {
    throw new Error('Japan government information result differs from the exact scenario')
  }
  const status = commandJson('hat', 'status', '--repository-id', repositoryId,
    '--context-partition-id', contextPartitionId, '--invocation-id', invocationId)
  if (status.status?.phase !== 'completed'
    || status.result?.output?.digest_sha256 !== receipt.outputDigestSha256) {
    throw new Error('Hatter invocation status differs from the worker result')
  }
  process.stdout.write(`${JSON.stringify({
    schema: 'hathq://hat-japan-government-information/owner-local-scenario-result/v1',
    invocationId,
    phase: status.status.phase,
    output,
  }, null, 2)}\n`)
} finally {
  await rm(root, { recursive: true, force: true })
}

function commandJson(...args) {
  const result = spawnSync(hatter, args, {
    env: { PATH: process.env.PATH, HATTER_HOME: hatterHome },
    encoding: 'utf8',
  })
  if (result.status !== 0) throw new Error(result.stderr)
  return JSON.parse(result.stdout)
}

async function waitForWorker(workerId) {
  const deadline = Date.now() + 5_000
  while (Date.now() < deadline) {
    try {
      const value = commandJson('hat', 'worker-read', '--repository-id', repositoryId,
        '--context-partition-id', contextPartitionId)
      if (value.worker?.worker_id === workerId || value.worker_id === workerId) return
    } catch {}
    await new Promise(resolve => setTimeout(resolve, 50))
  }
  throw new Error('Japan government information worker did not register')
}

function collect(child) {
  const stdout = []
  const stderr = []
  child.stdout.on('data', value => stdout.push(value))
  child.stderr.on('data', value => stderr.push(value))
  return new Promise((resolve, reject) => {
    child.once('error', reject)
    child.once('exit', code => resolve({ code,
      stdout: Buffer.concat(stdout).toString('utf8'),
      stderr: Buffer.concat(stderr).toString('utf8') }))
  })
}

function digest(value) {
  return createHash('sha256').update(value).digest('hex')
}
