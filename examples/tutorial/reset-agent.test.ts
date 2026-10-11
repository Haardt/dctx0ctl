import {expect, test} from 'bun:test';
import {mkdtemp, mkdir, readFile, readdir, rm, symlink, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
const root = resolve(import.meta.dir, '../..');
async function reset(step: string, directory: string, extra: string[] = []) {
  const child = Bun.spawn(['bun', join(import.meta.dir, 'reset-agent.ts'), step, '--directory', directory, ...extra], {
    cwd: root, stdout: 'pipe', stderr: 'pipe',
  });
  const [output, error, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  return {output, error, exit};
}
test('reset restores previous code and current tests, clears both databases and stale approvals, preserves guidance', async () => {
  const base = await mkdtemp(join(tmpdir(), 'tutorial-reset-test-'));
  const directory = join(base, 'exercise');
  try {
    expect((await reset('4', directory)).exit).toBe(0);
    await writeFile(join(directory, 'src/expressions.ts'), 'agent solution');
    await writeFile(join(directory, 'src/data.ts'), 'changed previous code');
    for (const path of ['.dctx0ctl', 'dctx0/.dctx0ctl']) {
      await mkdir(join(directory, path));
      await writeFile(join(directory, path, 'state.sqlite3'), 'old tasks and evidence');
    }
    await writeFile(join(directory, '.dctx0/testing/typescript/inspection-notes.json'), '{"approved":true}');
    await writeFile(join(directory, 'AGENTS.md'), 'Custom instructions');
    await writeFile(join(directory, '.dctx0/profiles/typescript/common.md'), 'Custom architecture');
    expect((await reset('4', directory)).exit).toBe(0);
    expect(await readFile(join(directory, 'AGENTS.md'), 'utf8')).toBe('Custom instructions');
    expect(await readFile(join(directory, '.dctx0/profiles/typescript/common.md'), 'utf8')).toBe('Custom architecture');
    expect(await readFile(join(directory, 'src/data.ts'), 'utf8')).toBe(await readFile(join(root, 'examples/tutorial-step-3-data/src/data.ts'), 'utf8'));
    expect(await Bun.file(join(directory, 'src/expressions.ts')).exists()).toBe(false);
    expect(await Bun.file(join(directory, 'src/expressions.test.ts')).exists()).toBe(true);
    expect(await Bun.file(join(directory, 'dctx0/.dctx0ctl/state.sqlite3')).exists()).toBe(false);
    expect(await Bun.file(join(directory, '.dctx0ctl/state.sqlite3')).exists()).toBe(false);
    expect(await Bun.file(join(directory, '.dctx0/testing/typescript/inspection-notes.json')).exists()).toBe(false);
  } finally { await rm(base, {recursive: true, force: true}); }
});
test('empty mode removes inherited production while preserving current acceptance tests', async () => {
  const base = await mkdtemp(join(tmpdir(), 'tutorial-reset-empty-'));
  try {
    const directory = join(base, 'exercise');
    expect((await reset('4', directory, ['--empty'])).exit).toBe(0);
    const files = await readdir(join(directory, 'src'));
    expect(files.every(name => name.endsWith('.test.ts'))).toBe(true);
    expect(files).toContain('expressions.test.ts');
    expect(await readFile(join(directory, 'TASK.md'), 'utf8')).toContain('production starts empty');
  } finally {await rm(base, {recursive: true, force: true});}
});
test('all creates sixteen independently resettable exercises including every final-step spec', async () => {
  const base = await mkdtemp(join(tmpdir(), 'tutorial-reset-all-'));
  try {
    expect((await reset('all', join(base, 'exercises'))).exit).toBe(0);
    expect(await readdir(join(base, 'exercises'))).toHaveLength(16);
    expect((await readdir(join(base, 'exercises/step-1/src'))).sort()).toEqual(['board.test.ts']);
    expect((await readdir(join(base, 'exercises/step-16/dctx0'))).sort()).toEqual(['copy.dctx0', 'http.dctx0', 'verification.dctx0', 'workspace.dctx0']);
    expect(await Bun.file(join(base, 'exercises/step-16/server.test.ts')).exists()).toBe(true);
    expect(await Bun.file(join(base, 'exercises/step-16/server.ts')).exists()).toBe(false);
  } finally { await rm(base, {recursive: true, force: true}); }
});
test('unmarked directories, references, mismatched steps and symlinks cannot be reset', async () => {
  const base = await mkdtemp(join(tmpdir(), 'tutorial-reset-guards-'));
  try {
    expect((await reset('1', base)).exit).not.toBe(0);
    expect((await reset('1', join(root, 'examples/tutorial-step-1-values'))).exit).not.toBe(0);
    expect((await reset('1', join(base, 'exercise'))).exit).toBe(0);
    expect((await reset('2', join(base, 'exercise'))).exit).not.toBe(0);
    await symlink(join(base, 'exercise'), join(base, 'link'));
    expect((await reset('1', join(base, 'link'))).exit).not.toBe(0);
    expect((await reset('0', join(base, 'invalid'))).exit).not.toBe(0);
  } finally { await rm(base, {recursive: true, force: true}); }
});
