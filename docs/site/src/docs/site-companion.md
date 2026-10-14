# Static Site and Task Live View

The companion works with **any dctx0 project** that has a valid published
specification. It reads the language-neutral spec, task and evidence history;
it does not depend on the application's target language or framework. Cinema
illustrates the interface below.

![Architecture with implementation filtering, dependency focus and a floating Inspector](../assets/dctx0-static-site.png)

## Generate a site for your project

Replace `path/to/project/dctx0` with the directory containing your specification:

```sh
# Publish a valid spec snapshot first.
dctx0 check path/to/project/dctx0

# Optional: populate task navigation and the task graph.
dctx0 tasks plan path/to/project/dctx0

# Export a frozen, portable site.
dctx0 site export path/to/project/dctx0 --output target/project-site

# Or start the local live companion.
dctx0 site serve path/to/project/dctx0
```

Open `target/project-site/index.html` directly, or publish the exported directory
on a static host. All assets and captured data are embedded in the HTML, so
`file://` and offline browsing work without Bun, Node or a runtime service.

For live mode, open the printed `http://127.0.0.1:…` URL. An available port is
chosen by default; use `--port 4179` for a fixed port. Task and evidence updates
made through the CLI appear every three seconds while the tab is visible.
Selection, filters and graph positions survive refresh. Ctrl-C stops the server.

The site only reads history: it does not run checks, create plans, execute work,
repair databases or approve evidence. Missing plans or stored runs leave the
corresponding sections empty. A missing published spec produces an error; run
`check` explicitly first. For a custom database, use the same global `--db`
path for publication and site commands. Existing CLI snapshot/history commands
remain the source of publication and task transitions.

## Explore the architecture

Start with **Implementation** to see domain/module groups, surfaces, ports,
operations and data contracts. Examples, tests, proofs, reviews, policies and
other spec-only declarations are hidden along with their connections. Choose
**All spec nodes** to include them again. The filter also applies to focus search
and selected-node details.

Use the search inside **Focus node**, or select a graph node and choose
**Focus selected**. **Direction** controls what appears around the focus:

| Direction | Visible neighborhood |
| --- | --- |
| Children | The node and descendants following declared parent links |
| Uses / calls | Outgoing semantic dependencies |
| Used / called by | Incoming semantic dependencies |
| Both directions | Incoming and outgoing dependencies |

The **Depth** slider selects 0–5 levels, with **All** at its last stop. Zero shows
only the focus node. Children mode counts hierarchy levels; dependency modes
count relationship hops. A group's own children remain visible to the chosen
depth, and operations show their declaring surface/port as context above depth
zero. For example, focusing `FilmCommunityApi` retains `getFilmRating`; focusing
that operation retains its owning surface. Semantic cycles are allowed.

**Show all** returns to the expanded overview. Drag nodes to arrange them; use
pan, zoom and Fit to navigate. Parallel connections are bundled, with their
counts and original relations available in the Inspector. Focus, direction,
depth and node filter are stored in the URL hash, so views can be shared.
Personal node positions and viewports are kept in browser storage.

## Inspect without leaving the graph

Select a node or connection to open the narrow, modeless **Inspector** over the
graph, or reopen it with the **Inspector** button. Its contents scroll internally,
so you do not need to scroll below the architecture view to read details.

Drag the header to move it, or focus the header and use arrow keys (Shift moves
in larger steps). The right-aligned **−** and **×** buttons minimize and close
it; **+** restores a minimized Inspector. Position is kept locally and constrained
to the graph area when the screen is resized. The panel does not block the rest
of the graph or trap keyboard focus.

**Selection** shows the selected declaration or connection. **Connections**
offers a list of connection buttons, also reachable by keyboard, with each
individual relationship and reason. Child declaration links select within the
architecture view; source links open the exact pinned source and line.

## Tasks and evidence

Task graphs start with compact, expandable waves. Task focus follows actual
prerequisites, with delivered results and blockers; optional history links are
separate from scheduling dependencies. Work completion does not imply testing
PASS. Saved native runs, imported assessments and source-bound reviews preserve
their own provenance. Historical evidence never proves working-tree freshness.

Overview, Spec, Behavior, Changes, Evidence and Stats expose the same published
project data in both export and live modes. Offline exports stay frozen; live
connection failures leave the last snapshot visible with a stale status.

## Data, export and capability boundaries

`dctx0 site export <spec-root> --output <directory>` writes a self-contained React
companion, `snapshot.json`, and installed dependency notices. Open `index.html`
directly with `file://`; no runtime server, Bun, Node, CDN, or network is needed.
Existing unrelated files and edits to generated files are preserved. A locked
export manifest tracks managed file hashes; collisions fail before replacement.
`provenance.json` carries the embedded asset receipt. Individual files use durable atomic
replacement; the self-contained HTML is published last and is its own coherent
commit point. Consumers of separate JSON read it as one document.

`dctx0 site serve <spec-root> [--port 0]` binds Axum exclusively to `127.0.0.1`.
Port zero selects a free port; the CLI prints its URL and stops on Ctrl-C.
Only `/` and read-only `GET /api/site` exist. ETags return 304 for unchanged data.
Global `--db` applies to both commands. Export also accepts `--snapshot <hash>`,
`--plan <integer>`, and `--as-of <event-sequence>`. Pins are independent and
mismatches are visible. `--as-of` selects task history, not an inferred timestamp
or a verification execution cutoff.

Spec, task session, original sources, checks, explicit renames, semantic changes
and stored verification runs are read in one SQLite transaction. Schema 1 has no
tasks; schemas 1–3 have no run history. Missing, newer, corrupt or unpublished
histories fail without migration, repair, compilation or database creation.
Failed later checks never replace a valid spec head.

Version-1 `SiteSnapshot` retains typed Rust models. Run summaries include exact case counts and inventory completeness; complete
case records and required IDs are packed per run as
`gzip-base64-json-decimal-integers-v2` and decoded only on selection with the
browser DecompressionStream API. All original observations and input values
remain available offline. The live server caches a coherent body by immutable
check/plan/event/run/rename watermarks, avoiding repeated full case serialization
for unchanged conditional requests. Within `records`, `tasks` and
`dependencies`, **all integer scalars are exact decimal strings**, including IDs,
claims, versions, sequences, spans, budgets and observed metrics. Top-level
`schema_version` remains numeric. CLI/IR/SQLite formats are unchanged. Embedded
JSON escapes HTML delimiters so source text cannot terminate its script element.

Overview leads with module intent, active work, blockers and evidence. Spec and
behavior details expose original sources and modeled requirements. Task
readiness and delivered blockers come from the central Rust model. Full
requirements, results, checks, reviews, events and history load through selection
or disclosure. Changes retain diff facets, impacts and predecessor results.
Stats links counts to records and omits unavailable measurement series.

React Flow Community/Dagre provide task and semantic graphs. Architecture starts
with workspace/domain/module nodes, expanding declarations on selection.
Semantic cycles are allowed. Task focus follows actual prerequisites; optional
history edges are dashed and never scheduling dependencies. Node positions and
viewports persist locally, with fallback when browser storage is unavailable.
Keyboard-accessible relationship listings complement the graphs.

Each evidence square is a stored run group and its captured scope. Native and
imported origins remain visible; reviews stay separate task records. Green
requires stored PASS, matching snapshot/check, complete case/subject inventory
and no open obligations or incomplete provenance. FAIL is red, UNKNOWN/partial
is yellow, historical/not executed is gray. Cases expose actual inputs,
observations, issues and artifact/replay provenance. Display never authorizes
adoption or asserts current working-tree freshness.

Live polling runs every three seconds, pauses when hidden and installs a coherent
snapshot. Selection, filters, scroll and graph positions persist. Historical task
requirements retain their original snapshot frames and source links. Connection
failure retains the last data with a stale status. Offline exports never poll.
Long declaration/task/case lists are virtualized; small screens have a navigation
toggle. Labels are English; light/dark modes use neutral surfaces and status colors.

## Build and acceptance

```sh
bun install --frozen-lockfile --cwd ui/site
bun run --cwd ui/site check
bun run --cwd ui/site build
bun run --cwd ui/site verify-assets
python3 scripts/site_acceptance.py --samples 3 --iterations 5
python3 scripts/validate.py --coverage
```

Bun 1.4.2 builds an embedded IIFE and CSS. Committed
`ui/site/dist/provenance.json` pins source/lock hashes, Bun version, output hashes
and actual installed package licenses; `LICENSES.txt` retains runtime/build/test
notices. Rebuilding must reproduce every byte. Dependencies include OSS HeroUI
3.2.6, React Flow Community 12.12.0 and Dagre 3.1.1, with no Pro packages. Installed
version metadata, rather than current website descriptions, determines licenses.
The locked Cargo dependency inventory is recorded separately in
[`docs/site-rust-licenses.json`](../../../site-rust-licenses.json).
The export includes the installed UI dependency license texts.

The coupled runner uses isolated unchanged Cinema sources, actual checks/task
transitions and a bounded native contract execution. Rust acceptance covers
escaping, empty/old schemas, large IDs, pins, real changes/renames, export and HTTP
lifecycle. Locally installed Chromium/Playwright tests file navigation without
network, sources, graph movement/zoom/fit, mobile keyboard focus, ETags and real
CLI task refresh with stable selection/viewport. Explicitly synthetic stress
inputs test 10,000-task virtualization and are never published history.

Release measurements separate snapshot projection (`bench ... site-snapshot`),
export, server response, initial render and refresh. Rust phase RSS uses
`VmHWM`; process measurements also retain OS wait4 peak RSS. Chromium memory is
100ms sampled summed RSS of its owned process tree: shared pages count per
process and transient peaks may be missed. Refresh latency includes the polling
cadence. Compiler/build, core, server VmHWM and browser costs stay separate. Raw samples,
input hashes, toolchain/platform, dirty status, cache conditions and bundle bytes
are retained under `benchmarks/history/site`. Locked atomic history retains the
latest 20 runs, summary medians and compatible recent trends. Site workloads remain unassessed without an explicitly accepted
baseline; existing baselines are unchanged.

`Dctx0.Site` extends the product spec. Its testing-only adapter calls the real
snapshot/export APIs; browser requirements remain explicit inspection work.
Self-verification capture and validation hashes include the embedded UI and
source/lock inputs, excluding installed dependencies. Earlier product acceptance
remains historical and does not automatically cover this new area.

## Published Cinema demonstration

The 2026-10-13 demonstration reads the existing Cinema publication and 45 stored
runs into `target/cinema-site/index.html`. Its roughly 58 MB self-contained HTML
retains the full historical case inventory. This is a larger workload than the
isolated acceptance fixture: the final export measured 43,342 ms and 1,877,995,520
bytes peak child RSS while workspace validation was running. These are retained
observations, not a baseline comparison. Chromium opened the full export, selected
a stored case and issued zero remote requests; initial rendering took 1,054 ms
and its sampled process-tree RSS peaked at 1,216,069,632 bytes. Browser and export
metrics use different memory scopes and must not be compared directly.

The retained acceptance and workspace evidence is linked from
[`docs/site-companion-validation.json`](../../../site-companion-validation.json).

Final fresh workspace coverage passes 31,886/32,669 merged source lines (97.60%)
and 2,599/2,677 LLVM functions (97.09%), with matching source hashes. Raw LLVM
lines remain 96.95%, regions 94.83%, and instantiations 61.98%; these are separate
metrics, not a branch-completeness claim. No exclusions or lower gates were added.

The final coupled workspace run passes all functional stages and records 71
release workloads in `20261013T195347.552983Z-2148470.json`, with identical
source hashes. Its one-sample/one-iteration settings remain unassessed. The Site
acceptance uses three samples/five projection iterations and also retains matching
hashes. Existing accepted baselines were not promoted.

## Graph readability correction

Source-view styles use a dedicated `source-code` class so they cannot stretch
React Flow source handles. Both graphs use six-pixel handles facing left/right
and the same fixed node dimensions as the Dagre layout. Parallel connections
are bundled by direction and relationship category; selection retains every
original relation and prerequisite reason. History remains separate. Whole-plan
and active task graphs start with collapsible wave summaries; selected-task
focus shows the actual prerequisite closure. A new layout-storage namespace
keeps the old coordinates intact while starting with the corrected layout.

The final Site acceptance phase checks types, six model tests, reproducible
assets, Rust export/server boundaries, browser graph geometry, wave expansion,
refresh and release speed/RSS. This UI correction changes no Rust logic or
specification semantics; the earlier full-workspace coverage remains historical.

The browser phase stopped at a disabled Zoom In button in a maximum-zoom focus
view. Its failed/incomplete record is retained. After correcting that test
assumption, only the remaining browser scenario and unreached history checks
continued; passed Rust/model/build stages were not repeated. Combined correction
evidence and actual speed/RSS samples are retained in
[`docs/site-graph-correction-validation.json`](../../../site-graph-correction-validation.json).

Evidence scope uses the published `not_selected` inventory rather than comparing
one selected module with the number of declarations it contains. An original
native UNKNOWN with manual-only obligations is labeled REVIEWED only when a
separate stored PASS inspection has identical captured provenance and case data,
contains all original artifacts and includes a program-inspections artifact.
The original verdict remains inspectable and the effective assessment is linked;
review is never relabeled native testing PASS.

Evidence correction checks (eight model tests, typecheck, actual Cinema records,
offline browser navigation and export) pass with time/RSS samples in
[`docs/site-evidence-correction-validation.json`](../../../site-evidence-correction-validation.json).
Artifact inclusion uses a set lookup to keep inspection matching linear.

Architecture focus is separate from group expansion. The **Focus node** combobox
includes search with Arrow/Enter/Escape keyboard navigation; **Focus selected**
uses the selected graph node. **Direction** offers Children, Uses / calls,
Used / called by, and Both directions. Dependency views follow actual semantic
dependency edges, preserving their original kinds and endpoints in details;
containment and governing-fact edges do not count as dependency hops. A focused group keeps its own children to the selected depth. Dependency
endpoints are expanded to visible declarations; focused operations also show
their declaring surface/port as context at depth greater than zero. Explicit
contains edges preserve this ownership without adding dependency hops.

The **Depth** slider provides 0–5 levels and All at its last stop. Children mode
counts declared parent levels; dependency modes count directed relationship hops.
Cycles terminate through visited-node tracking. **Show all** restores the expanded
overview. Focus, direction and depth are stored in hash URLs; positions and
viewports are retained separately for each scope.

Focus validation and time/RSS measurements are retained in
[`docs/site-focus-validation.json`](../../../site-focus-validation.json). The coupled
Site runner includes `ui/site/focus-browser.ts` and retains its screenshot.

Direction/search/slider validation and measured time/RSS are retained in
[`docs/site-focus-relations-validation.json`](../../../site-focus-relations-validation.json).

Architecture defaults to the **Implementation** node filter: workspace/domain/module
groups, surfaces, ports, operations, entities, types, values, messages, events,
failures and externals. Examples, tests, proofs, reviews, policies, invariants,
lifecycles and decisions remain available through **All spec nodes**. Filtering
applies before dependency traversal, excluding verification edges rather than
folding them into implementation parents. Search and selected-node details use
the same filtered projection. Switching filters clears a focus/selection that is
no longer visible. The `nodes` hash parameter preserves the filter on reload.

Filter checks and actual time/RSS measurements are retained in
[`docs/site-architecture-filter-validation.json`](../../../site-architecture-filter-validation.json).

The Cinema regression checks both `FilmCommunityApi` → `getFilmRating` visibility
and the owning surface when focusing `getFilmRating` directly. Recorded validation
and time/RSS measurements: [surface focus evidence](../../../site-surface-focus-validation.json).

Architecture details open in a narrow, modeless **Inspector** over the graph.
Select a node or connection to inspect it; **Connections** provides keyboard-reachable
connection buttons under a familiar label. The inspector
scrolls its own contents. Drag its header, or focus the header and use arrow keys
(Shift for larger steps), to move it. Position is kept locally where storage is
available and clamped to the graph bounds after resizing. Minimize/expand and
close/reopen controls leave the graph usable. Child declaration links select within
Architecture; pinned source links still open the source view.

Inspector interaction checks and actual speed/RSS samples are retained in
[inspector validation](../../../site-inspector-validation.json).
