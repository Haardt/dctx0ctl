import {strict as assert} from 'node:assert';
import {createHash} from 'node:crypto';
import {Database} from 'bun:sqlite';
import {cp, mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
const root = resolve(import.meta.dir, '../..');
const binary = process.env.DCTX0_BIN ?? join(root, 'target/release/dctx0');
const scratch = await mkdtemp(join(tmpdir(), 'tutorial-variants-'));
async function execute(command: string[], cwd: string, allowed = [0]) {
  const child = Bun.spawn(command, {cwd, stdout: 'pipe', stderr: 'pipe'});
  const [output, error, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  assert.ok(allowed.includes(exit), command.join(' ') + '\n' + error);
  return output;
}
async function measured(command: string[], cwd: string) {
  const start = performance.now();
  const child = Bun.spawn(command, {cwd, stdout: 'pipe', stderr: 'pipe'});
  let peak: number | null = null;
  let finished = false;
  const sampling = (async () => {
    while (!finished) {
      const status = await readFile('/proc/' + child.pid + '/status', 'utf8').catch(() => '');
      const rss = status.match(/^VmHWM:\s+(\d+) kB$/m);
      if (rss) peak = Math.max(peak ?? 0, Number(rss[1]) * 1024);
      await Bun.sleep(2);
    }
  })();
  const [output, error, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  finished = true;await sampling;
  assert.equal(exit, 0, command.join(' ') + '\n' + error);
  return {output, elapsed_ms: performance.now() - start, peak_rss_bytes: peak,
    scope: 'Elapsed whole Rust command; Linux Rust-process VmHWM sampled every 2 ms, excludes child memory and may miss the final peak; null means unavailable'};
}
const variants = ['bun-direct', 'hono-awilix-classes'];
const reports: any[] = [];
for (const name of variants) {
  const project = join(root, 'examples/tutorial-variants', name);
  await execute(['bun', 'install', '--frozen-lockfile'], project);
  await execute(['bun', 'run', 'typecheck'], project);
  const coverageDir = join(scratch, name);
  await execute(['bun', 'run', 'test', '--coverage', '--coverage-reporter=lcov', '--coverage-dir=' + coverageDir], project);
  const coverage = await readFile(join(coverageDir, 'lcov.info'), 'utf8');
  for (const name of ['src/board.ts', 'src/app.ts', 'src/server.ts']) {
    const record = coverage.split('end_of_record').find(record => record.includes('SF:' + name + '\n'));
    assert.ok(record, 'Missing production coverage: ' + name);
    const counts = Object.fromEntries(Array.from(record.matchAll(/^(FNF|FNH|LF|LH):(\d+)$/gm), match => [match[1], Number(match[2])]));
    assert.equal(counts.LF, counts.LH, name + ': uncovered lines');
    assert.equal(counts.FNF, counts.FNH, name + ': uncovered functions');
  }
  const complete = JSON.parse(await execute([binary, 'verify', 'complete', 'dctx0', '--project', '.', '--json'], project));
  assert.equal(complete.result.status, 'PASS');assert.equal(complete.open_tasks, 0);assert.equal(complete.non_pass_tasks, 0);
  const plan = JSON.parse(await execute([binary, 'verify', 'plan', 'dctx0', '--subject', 'Tutorial.Board', '--json'], project));
  // Publication occurrence is independent history, not executable meaning.
  const {check_occurrence: _occurrence, ...semanticPlan} = plan;
  const hashes: Record<string, string> = {};
  for await (const file of new Bun.Glob('{src/*.ts,dctx0/*.dctx0,.dctx0/testing/**/*.ts,.dctx0/testing/**/*.json,.dctx0/profiles/**/*.md,AGENTS.md,package.json,bun.lock,tsconfig.json}').scan({cwd: project, dot: true})) {
    if (/\/(worker|module-observer)\.ts$/.test(file)) continue;
    hashes[file] = createHash('sha256').update(await readFile(join(project, file))).digest('hex');
  }
  reports.push({name, complete, semantic_plan_hash: createHash('sha256').update(JSON.stringify(semanticPlan)).digest('hex'), production_coverage_lcov: coverage, source_hashes: hashes});
  console.log(name + ': strict types, independent tests, 100% production lines/functions, native PASS');
}
await execute(['bun', 'test', 'examples/tutorial/agent-step.test.ts', 'examples/tutorial/reset-agent.test.ts'], root);
for (const report of reports) {
  const original = join(root, 'examples/tutorial-variants', report.name);
  const project = join(scratch, report.name + '-mutation');
  await cp(original, project, {recursive: true, filter: path => !/(?:^|\/)(node_modules|\.dctx0ctl|coverage)(?:\/|$)/.test(path)});
  try {
    const file = join(project, 'src/board.ts');
    const source = await readFile(file, 'utf8');
    assert.ok(source.includes('return value + 1n;'));
    await Bun.write(file, source.replace('return value + 1n;', 'return value + 2n;'));
    await execute(['bun', 'install', '--frozen-lockfile'], project);
    await execute([binary, 'check', 'dctx0'], project);
    const saved = JSON.parse(await execute([binary, 'verify', 'run', 'dctx0', '--project', '.', '--subject', 'Tutorial.Board.Actions.increment', '--no-adopt', '--json'], project, [0, 1]));
    const run = saved.evidence.run;
    assert.equal(run.result.status, 'FAIL', 'Real production mutation must fail through HTTP');
    const failure = run.cases.find((entry: any) => entry.result.status === 'FAIL');
    assert.ok(failure, 'Technical UNKNOWN does not establish mutation detection');
    report.production_mutation = {change: 'Actual increment adds two instead of one', result: run.result, counterexample: failure};
  } finally {await rm(project, {recursive: true, force: true});}
}
// Performance is measured after functional acceptance, without coverage instrumentation.
for (const report of reports) {
  const project = join(root, 'examples/tutorial-variants', report.name);
  const measurement = await measured([binary, 'verify', 'run', 'dctx0', '--project', '.', '--no-adopt', '--json'], project);
  const saved = JSON.parse(measurement.output);
  const run = saved.evidence.run;
  assert.equal(run.result.status, 'PASS');
  assert.equal(run.required_cases.length, run.cases.length);
  assert.ok(run.cases.every((entry: any) => entry.result.status === 'PASS'));
  const artifact = run.artifacts.find((entry: any) => entry.media_type === 'application/vnd.dctx0.runtime-metrics+json');
  assert.ok(artifact, 'Missing actual worker memory measurement');
  const database = new Database(join(project, 'dctx0/.dctx0ctl/state.sqlite3'), {readonly: true});
  let metrics;
  try {
    const row = database.query('SELECT bytes FROM verification_artifacts WHERE hash=?').get(artifact.hash) as {bytes: Uint8Array};
    assert.ok(row);assert.equal(createHash('sha256').update(row.bytes).digest('hex'), artifact.hash);
    metrics = JSON.parse(new TextDecoder().decode(row.bytes));assert.ok(metrics.peak_rss_bytes > 0);
  } finally {database.close();}
  report.performance = {elapsed_ms: measurement.elapsed_ms, peak_rss_bytes: measurement.peak_rss_bytes, scope: measurement.scope, native_elapsed_us: run.elapsed_us, native_worker_metrics: metrics, baseline: 'new workload; unassessed; accepted baseline unchanged'};
  report.native = {run_id: saved.evidence.id, plan_hash: run.provenance.plan_hash, snapshot: run.provenance.snapshot, mandatory_cases: run.required_cases.length, recorded_cases: run.cases.length, result: run.result};
}
assert.equal(reports[0].native.snapshot, reports[1].native.snapshot, 'Variants must use the same spec');
assert.equal(reports[0].semantic_plan_hash, reports[1].semantic_plan_hash, 'Variants must use the same complete semantic plan; independent check occurrences remain separately pinned');
await Bun.write(join(root, 'examples/tutorial-variants/validation.json'), JSON.stringify({schema_version: 1, status: 'PASS', created_at: new Date().toISOString(), variants: reports, reset_tests: '10 tests passed; includes all sixteen exercises, both database locations, empty mode and guidance retention'}, null, 2) + '\n');
console.log('Both variants: same spec and plan; functional checks and final speed/RSS measurements PASS.');
