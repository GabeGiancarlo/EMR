// Read-only cost and worker summary. Never loads or prints the API key.
import fs from 'node:fs';
import path from 'node:path';
const file = path.resolve(import.meta.dirname, '../.local/xai/usage.json');
if (!fs.existsSync(file)) {
  console.log('No local xAI usage ledger exists yet.');
  process.exit(0);
}
const ledger = JSON.parse(fs.readFileSync(file, 'utf8'));
let knownUsd = Number(ledger.priorReviewUsd ?? 0);
let reservedUsd = 0;
for (const item of ledger.requests) {
  if (typeof item.costUsd === 'number') knownUsd += item.costUsd;
  else reservedUsd += Number(item.reservedUsd ?? 0);
}
console.log(JSON.stringify({
  knownBilledUsd: Number(knownUsd.toFixed(6)),
  pendingOrUnknownReservationsUsd: reservedUsd,
  budgetExposureUsd: Number((knownUsd + reservedUsd).toFixed(6)),
  historicalCeilingUsd: ledger.authorizedCeilingUsd ?? null,
  activeRun: ledger.runScopedAuthorizationActive ?? null,
  runs: Object.entries(ledger.runs ?? {}).map(([runId, authorization]) => {
    const requests = ledger.requests.filter(r => r.runId === runId);
    const billed = requests.reduce((sum, r) => sum + (typeof r.costUsd === 'number' ? r.costUsd : 0), 0);
    const holds = requests.reduce((sum, r) => sum + (typeof r.costUsd === 'number' ? 0 : Number(r.reservedUsd ?? 0)), 0);
    return { runId, userCeilingUsd: authorization.authorizedCeilingUsd,
      knownBilledUsd: Number(billed.toFixed(6)), pendingOrUnknownReservationsUsd: holds,
      budgetExposureUsd: Number((billed + holds).toFixed(6)) };
  }),
  note: 'Reservations are conservative budget holds, not confirmed charges. Account billing is authoritative.',
  workers: ledger.requests.map(r => ({
    name: r.name, runId: r.runId ?? 'historical', status: r.status, validation: r.validation,
    elapsedMinutes: r.status === 'pending' ? Math.round((Date.now() - Date.parse(r.started)) / 6000) / 10 : undefined,
    billedUsd: r.costUsd,
  })),
}, null, 2));
