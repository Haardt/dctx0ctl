<div class="landing">
<nav class="landing-nav" aria-label="Product navigation">
<a class="wordmark" href="index.html" aria-label="DCTX0 home"><span class="brand-mark" aria-hidden="true">&lt;◇&gt;</span> DCTX0</a>
<div><a href="docs/language.html">Language</a><a href="docs/getting-started.html">Docs</a><a href="https://github.com/Haardt/dctx0ctl">GitHub <span aria-hidden="true">↗</span></a></div>
</nav>

<section class="hero" aria-labelledby="hero-title">
<p class="eyebrow">Executable specifications for coding agents</p>
<h1 id="hero-title">Make intent<br><span>executable.</span></h1>
<p class="hero-lead">Describe your product.<br>Review the contract your agent writes.</p>
<p class="hero-description">Start in plain language. Your coding agent writes and validates the specification with you; you review its rules and expected outcomes. Then implement focused tasks and verify the real application.</p>
<div class="hero-actions"><a class="primary-action" href="docs/getting-started.html">Get started <span aria-hidden="true">→</span></a><a class="secondary-action" href="#specification">See the specification</a></div>
<div class="target-strip" aria-label="Executable targets"><span>Native verification</span><strong>TypeScript / Bun</strong><strong>Go</strong><strong>Rust</strong></div>
</section>

<section class="intent-section" aria-labelledby="intent-title">
<div class="section-label">01 / Understand first</div>
<div class="split-section"><div><h2 id="intent-title">More of the product.<br>Less implementation noise.</h2></div><div><p>You can start without learning to author DCTX0 yourself. Your agent handles syntax and validation while you discuss domain rules, relationships, and missing decisions. Familiar types, ranges, and given/when/expect examples help developers read the contract.</p><p>Keeping implementation details separate can leave more context for the application's meaning. People can understand and review the behavior before reading its database plumbing or framework wiring.</p><a class="text-link" href="docs/authoring.html">Author a contract with your agent <span aria-hidden="true">→</span></a></div></div>
</section>

<section class="spec-section" id="specification" aria-labelledby="spec-title">
<div class="section-label">02 / Specify what must hold</div>
<div class="split-section"><div><h2 id="spec-title">A contract.<br>An observable outcome.</h2><p>This Cinema example describes a missing film. An external adapter supplies the declared store reply and invokes the real application. The Rust core checks the result.</p><p>Algorithms stay in application code. Domain rules become the reference for implementation and testing.</p><a class="text-link" href="docs/tutorial.html">Build a complete specification <span aria-hidden="true">→</span></a></div><div class="spec-code">
<div class="code-caption"><span>movie-night.dctx0</span><span>Excerpt / FilmCatalog</span></div>

```dctx0
example "unknown film details" {
  given FilmCatalogStore.find(filmId)
    returns None
  when result = FilmCatalogApi.getFilm(filmId)
  expect result fails FilmNotFound
}
```

<p class="code-note">The module declares the port, surface, and failure. A typed fixture supplies <code>filmId</code>.</p>
</div></div>
</section>

<section class="workflow-section" aria-labelledby="workflow-title">
<div class="section-label">03 / From meaning to implementation</div>
<h2 id="workflow-title">One contract. A focused workflow.</h2>
<ol class="workflow-line"><li><span>01</span><h3>Specify</h3><p>Develop the contract with your agent. Review the product decisions. Compile the specification.</p></li><li><span>02</span><h3>Implement</h3><p>Work through tasks with explicit prerequisites, needed types, and pinned acceptance criteria.</p></li><li><span>03</span><h3>Verify</h3><p>Execute the application. Assess observations centrally. Repair counterexamples and replay them.</p></li></ol>
<a class="text-link" href="docs/workflow.html">Explore the CLI workflow <span aria-hidden="true">→</span></a>
</section>

<section class="evidence-section" aria-labelledby="evidence-title">
<div class="section-label">04 / Know what was established</div>
<div class="split-section"><div><h2 id="evidence-title">Evidence you can inspect.</h2><p>Workers report observations, not their own verdicts. Missing evidence stays visible. An authored review cannot approve away a failed native check.</p><a class="text-link" href="docs/evidence.html">Understand evidence and replay <span aria-hidden="true">→</span></a></div><dl class="evidence-ledger"><div><dt><span class="status-dot pass"></span>PASS</dt><dd>The selected finite requirements are satisfied.</dd></div><div><dt><span class="status-dot fail"></span>FAIL</dt><dd>An observed result violates the contract.</dd></div><div><dt><span class="status-dot unknown"></span>UNKNOWN</dt><dd>A required statement still needs evidence.</dd></div><div><dt>Source-bound review</dt><dd>A separate authored judgment, pinned to the inspected inputs.</dd></div></dl></div>
</section>

<section class="cinema-section" aria-labelledby="cinema-title">
<div class="section-label">05 / Real applications, shared semantics</div>
<h2 id="cinema-title">Same Cinema contract.<br>Three native implementations.</h2>
<p>TypeScript, Go, and Rust run a shared plan with <strong>5,674 mandatory finite cases</strong>. Documented acceptances exercise persistence, HTTP, deliberate production mutations, and broken/repaired replay. Manual requirements retain separate inspection evidence.</p>
<div class="cinema-links"><a href="docs/targets.html#typescript--bun">TypeScript / Bun <span aria-hidden="true">↗</span></a><a href="docs/targets.html#go">Go <span aria-hidden="true">↗</span></a><a href="docs/targets.html#rust">Rust <span aria-hidden="true">↗</span></a></div>
</section>

<section class="start-section" aria-labelledby="start-title"><p class="eyebrow">Your next step</p><h2 id="start-title">Start with what<br>your software means.</h2><div class="hero-actions"><a class="primary-action" href="docs/getting-started.html">Build your first contract <span aria-hidden="true">→</span></a><a class="secondary-action" href="docs/miniboard.html">Learn with working examples</a></div></section>

<footer class="landing-footer"><span>DCTX0 · Make intent executable.</span><div><a href="docs/tools.html">Skills &amp; editors</a><a href="docs/sharing.html">Source &amp; history</a><a href="https://github.com/Haardt/dctx0ctl/blob/main/LICENSE">MIT license</a></div></footer>
</div>
