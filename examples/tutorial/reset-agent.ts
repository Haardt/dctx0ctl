import {cp, lstat, mkdir, mkdtemp, readFile, readdir, rename, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, join, resolve, sep} from 'node:path';

const root = resolve(import.meta.dir, '../..');
const args = process.argv.slice(2);
const selected = args.shift();
if (!selected || !/^(all|[1-9]|1[0-6])$/.test(selected)) {
  throw new Error('Usage: bun examples/tutorial/reset-agent.ts <1..16|all> [--empty] [--directory <exercise or collection directory>]');
}
const empty = args.includes('--empty');
if (empty) args.splice(args.indexOf('--empty'), 1);
let location = resolve(root, 'target/tutorial-exercises');
if (args.length) {
  if (args.length !== 2 || args[0] !== '--directory') throw new Error('Expected --directory <path>');
  location = resolve(args[1]);
} else if (selected !== 'all') location = join(location, 'step-' + selected);
const steps = selected === 'all' ? Array.from({length: 16}, (_, index) => index + 1) : [Number(selected)];

async function reset(step: number, directory: string) {
  // Never allow replacement of the checkout, its ancestors or reference examples.
  if (directory === root || root.startsWith(directory + sep) ||
      directory === join(root, 'examples') || directory.startsWith(join(root, 'examples') + sep)) {
    throw new Error('Reference examples and checkout ancestors cannot be reset: ' + directory);
  }
  const existing = await lstat(directory).catch(error => {
    if (error.code !== 'ENOENT') throw error;
    return null;
  });
  if (existing) {
    if (!existing.isDirectory() || existing.isSymbolicLink()) throw new Error('Exercise must be a real directory');
    const marker = JSON.parse(await readFile(join(directory, '.tutorial-exercise.json'), 'utf8').catch(() => {
      throw new Error('Refusing to replace an unmarked directory: ' + directory);
    }));
    if (marker.schema_version !== 1 || marker.step !== step) throw new Error('Exercise marker does not match step ' + step);
  }
  const scratch = await mkdtemp(join(tmpdir(), 'dctx0-reset-agent-'));
  try {
    const child = Bun.spawn(['bash', join(import.meta.dir, 'agent-step.sh'), String(step)], {
      cwd: root, env: {...process.env, TMPDIR: scratch}, stdout: 'pipe', stderr: 'pipe',
    });
    const [output, error, exit] = await Promise.all([
      new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited,
    ]);
    if (exit !== 0) throw new Error('Cannot prepare exercise: ' + error);
    const fresh = output.match(/^Exercise: (.+)$/m)?.[1];
    if (!fresh) throw new Error('Exercise generator returned no directory');
    if (empty) {
      for (const name of await readdir(join(fresh, 'src'))) {
        if (!name.endsWith('.test.ts')) await rm(join(fresh, 'src', name), {recursive: true, force: true});
      }
      for (const name of ['AGENTS.md', 'TASK.md']) {
        const file = join(fresh, name);
        const text = await readFile(file, 'utf8');
        await Bun.write(file, text + '\nReset mode: production starts empty. Implement the entire current specification, including inherited requirements.\n');
      }
    }
    // Preserve the agent experiment's architecture choices, not old test approvals.
    if (existing) {
      for (const relative of ['AGENTS.md', '.dctx0/profiles']) {
        const source = join(directory, relative);
        const info = await lstat(source).catch(() => null);
        if (info) {
          if (info.isSymbolicLink()) throw new Error('Refusing a symlinked guidance path: ' + source);
          await rm(join(fresh, relative), {recursive: true, force: true});
          await cp(source, join(fresh, relative), {recursive: true, dereference: false});
        }
      }
      await rm(join(fresh, '.dctx0/profiles/profiles.lock'), {force: true});
    }
    await mkdir(dirname(directory), {recursive: true});
    // Prepare beside the destination so the publish rename stays on one filesystem.
    const staging = await mkdtemp(join(dirname(directory), '.tutorial-reset-'));
    try {
      await cp(fresh, join(staging, 'ready'), {recursive: true});
      if (existing) await rename(directory, join(staging, 'previous'));
      try { await rename(join(staging, 'ready'), directory); }
      catch (error) {
        if (existing) await rename(join(staging, 'previous'), directory);
        throw error;
      }
    } finally { await rm(staging, {recursive: true, force: true}); }
    console.log('Reset step ' + step + ': ' + directory);
  } finally { await rm(scratch, {recursive: true, force: true}); }
}
for (const step of steps) await reset(step, selected === 'all' ? join(location, 'step-' + step) : location);
