// Added by HATHQ, 2026.
// Purpose: prepare one short-lived signed owner-local execution directory for this HAT.

import { createHash, createPublicKey, generateKeyPairSync, sign, verify } from 'node:crypto'
import { copyFile, lstat, mkdir, readFile, realpath, writeFile } from 'node:fs/promises'
import { dirname, isAbsolute, join, relative, resolve } from 'node:path'

const [sourceRoot, outputRoot, catalogKeyId, catalogPublicKeyHex] = process.argv.slice(2)
if (![sourceRoot, outputRoot].every(value => isAbsolute(value ?? ''))
  || !/^[a-z0-9][a-z0-9-]{0,95}$/u.test(catalogKeyId ?? '')
  || !/^[0-9a-f]{64}$/u.test(catalogPublicKeyHex ?? '')) {
  throw new Error('usage: prepare-owner-local-federation SOURCE OUTPUT CATALOG_KEY_ID CATALOG_PUBLIC_KEY_HEX')
}
await exactDirectory(sourceRoot)
await mkdir(outputRoot, { mode: 0o700 })
await exactDirectory(outputRoot)

const indexSource = join(sourceRoot, 'catalog/v2/index.json')
const signatureSource = join(sourceRoot, 'catalog/v2/index.signature.hex')
const indexBytes = await readFile(indexSource)
const signature = (await readFile(signatureSource, 'utf8')).trim()
const index = JSON.parse(indexBytes)
if (index.schema !== 'hathq://official-hats/catalog/v2'
  || index.origin !== 'https://ihat.space'
  || index.signing_key_id !== catalogKeyId
  || !/^[0-9a-f]{128}$/u.test(signature)
  || !verify(null, indexBytes, publicKey(catalogPublicKeyHex), Buffer.from(signature, 'hex'))) {
  throw new Error('official catalog trust verification failed')
}
const entry = index.entries?.find(value =>
  value.repository_id === 'hat-japan-government-information')
if (!entry) throw new Error('Japan government information HAT is absent from the catalog')
const packageSource = resolve(sourceRoot, entry.artifact_path.slice(1))
if (relative(sourceRoot, packageSource).startsWith('..')) {
  throw new Error('catalog package path escapes the source root')
}
const packageBytes = await readFile(packageSource)
if (digest(packageBytes) !== entry.package_sha256) {
  throw new Error('Japan government information HAT package differs from the signed catalog')
}
const hatPackage = JSON.parse(packageBytes)
if (hatPackage.repository_id !== entry.repository_id
  || hatPackage.package_id !== entry.package_id
  || hatPackage.version !== entry.version) {
  throw new Error('Japan government information HAT identity differs from the catalog')
}

await mkdir(join(outputRoot, 'catalog/v2'), { recursive: true, mode: 0o700 })
await copyFile(indexSource, join(outputRoot, 'catalog/v2/index.json'))
await copyFile(signatureSource, join(outputRoot, 'catalog/v2/index.signature.hex'))
const packageTarget = join(outputRoot, entry.artifact_path)
await mkdir(dirname(packageTarget), { recursive: true, mode: 0o700 })
await writeFile(packageTarget, packageBytes, { mode: 0o600, flag: 'wx' })

const now = Math.floor(Date.now() / 1000)
const expires = now + 3_600
const location = {
  schema: 'hathq://hat/execution-location/v1',
  location_id: 'owner-local-japan-government-information',
  package_id: entry.package_id,
  package_digest_sha256: entry.package_sha256,
  publisher_id: 'hathq',
  execution_kind: 'owner-local',
  worker_service_id: 'hat-japan-government-information-worker',
  identity_authority_ref: 'ihat/owner-local',
  transport_profile_ref: 'crowsi/owner-local-process-v1',
  route_ref: 'crowsi/owner-local/hat-japan-government-information',
  region: 'local',
  jurisdictions: ['jp'],
  data_residencies: ['local'],
  operation_ids: hatPackage.operations.map(value => value.id),
  accepted_classifications: hatPackage.manifest.input_classifications,
  capability_ids: hatPackage.manifest.capabilities,
  assurance: 'verified',
  issued_at_epoch_s: now,
  expires_at_epoch_s: expires,
  revocation_epoch: 0,
}
const locations = Buffer.from(JSON.stringify({
  schema: 'hathq://hat/execution-location-set/v1',
  package_id: entry.package_id,
  package_digest_sha256: entry.package_sha256,
  revision: 1,
  issued_at_epoch_s: now,
  expires_at_epoch_s: expires,
  locations: [location],
}))
const locationPath = '/federation/v1/locations/hat-japan-government-information.json'
const federationKeyId = 'official-hats-federation-v1'
const { privateKey, publicKey: federationPublicKey } = generateKeyPairSync('ed25519')
const directory = Buffer.from(JSON.stringify({
  schema: 'hathq://official-hats/federation-directory/v1',
  directory_id: 'official-hats',
  signing_key_id: 'official-hats-federation-v1',
  origin: index.origin,
  catalog_digest_sha256: digest(indexBytes),
  revision: 1,
  issued_at_epoch_s: now,
  expires_at_epoch_s: expires,
  maximum_child_depth: 1,
  children: [],
  location_sets: [{
    package_id: entry.package_id,
    package_digest_sha256: entry.package_sha256,
    document_path: locationPath,
    document_digest_sha256: digest(locations),
    location_count: 1,
  }],
}))
await mkdir(join(outputRoot, 'federation/v1/locations'), { recursive: true, mode: 0o700 })
await writeFile(join(outputRoot, locationPath), locations, { mode: 0o600, flag: 'wx' })
await writeFile(join(outputRoot, 'federation/v1/directory.json'), directory,
  { mode: 0o600, flag: 'wx' })
await writeFile(join(outputRoot, 'federation/v1/directory.signature.hex'),
  `${sign(null, directory, privateKey).toString('hex')}\n`, { mode: 0o600, flag: 'wx' })
const publicDer = federationPublicKey.export({ format: 'der', type: 'spki' })
const trust = {
  schema: 'hathq://hat-japan-government-information/owner-local-federation/v1',
  catalogSource: outputRoot,
  catalogSigningKeyId: catalogKeyId,
  catalogPublicKeyHex,
  federationSigningKeyId: federationKeyId,
  federationPublicKeyHex: publicDer.subarray(publicDer.length - 32).toString('hex'),
  locationId: location.location_id,
  expiresAtEpochS: expires,
}
await writeFile(join(outputRoot, 'owner-local-trust.json'),
  `${JSON.stringify(trust, null, 2)}\n`, { mode: 0o600, flag: 'wx' })
process.stdout.write(`${JSON.stringify(trust)}\n`)

function digest(value) {
  return createHash('sha256').update(value).digest('hex')
}

function publicKey(rawHex) {
  const prefix = Buffer.from('302a300506032b6570032100', 'hex')
  return createPublicKey({ key: Buffer.concat([prefix, Buffer.from(rawHex, 'hex')]),
    format: 'der', type: 'spki' })
}

async function exactDirectory(path) {
  const [entry, physical] = await Promise.all([lstat(path), realpath(path)])
  if (!entry.isDirectory() || entry.isSymbolicLink() || physical !== path) {
    throw new Error('directory must be one exact absolute directory')
  }
}
