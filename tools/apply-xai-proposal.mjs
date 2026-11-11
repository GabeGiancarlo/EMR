// Validate every proposed edit first. --apply is a separate, explicit local review step.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '..');
const [input, mode] = process.argv.slice(2);
if (!input || (mode && mode !== '--apply')) throw Error('Usage: node tools/apply-xai-proposal.mjs proposal.json [--apply]');
const proposal = JSON.parse(fs.readFileSync(input, 'utf8'));
const changes = new Map();
function resolve(file) {
  if (typeof file !== 'string' || !/^(api\/(src|tests)\/|frontend\/src\/|migrations\/|docs\/)/.test(file) || file.split(/[\\/]/).includes('..') || file.includes('\\')) throw Error(`Out-of-scope proposal path: ${file}`);
  const target = path.resolve(root, file);
  if (!target.startsWith(root + path.sep)) throw Error('Path escapes checkout');
  return target;
}
for (const file of proposal.files ?? []) {
  const target = resolve(file.path);
  if (fs.existsSync(target) || changes.has(file.path)) throw Error(`New file already exists: ${file.path}`);
  if (typeof file.content !== 'string' || file.content.length > 1000000) throw Error('Invalid proposed content');
  changes.set(file.path, { target, before: null, after: file.content });
}
for (const edit of proposal.edits ?? []) {
  const target = resolve(edit.path);
  const current = changes.get(edit.path) ?? { target, before: fs.readFileSync(target, 'utf8'), after: fs.readFileSync(target, 'utf8') };
  if (!edit.search || typeof edit.replace !== 'string') throw Error('Empty search or invalid replacement');
  const normalized = current.after.replace(/\r\n/g, '\n');
  const search = edit.search.replace(/\r\n/g, '\n');
  const occurrences = normalized.split(search).length - 1;
  if (occurrences !== 1) throw Error(`${edit.path}: expected one exact match, found ${occurrences}`);
  current.after = normalized.replace(search, () => edit.replace);
  changes.set(edit.path, current);
}
if (!changes.size) throw Error('No code changes');
for (const [file, change] of changes) {
  console.log(JSON.stringify({ file, newFile: change.before === null, characters: change.after.length, applied: mode === '--apply' }));
}
if (mode === '--apply') {
  const backup = path.join(root, '.local', 'xai', 'applied', `${Date.now()}-${path.basename(input)}`);
  fs.mkdirSync(path.dirname(backup), { recursive: true });
  fs.writeFileSync(backup, JSON.stringify([...changes.entries()].map(([file, c]) => ({ file, before: c.before })), null, 2));
  for (const change of changes.values()) {
    fs.mkdirSync(path.dirname(change.target), { recursive: true });
    fs.writeFileSync(change.target, change.after);
  }
}
