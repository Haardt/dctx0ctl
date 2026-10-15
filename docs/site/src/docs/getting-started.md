# Getting started

Describe your product in plain language and let your coding agent draft and validate the contract. You review the rules and expected outcomes. You can start without learning to author DCTX0 yourself.

## Install a released binary

```sh
curl -fsSL https://github.com/Haardt/dctx0ctl/releases/latest/download/install.sh | bash
export PATH="$HOME/.local/bin:$PATH"
dctx0 --version
```

The installer supports Linux x86_64/ARM64 and macOS Intel/Apple Silicon with glibc 2.35+, verifies checksums and the executable version, and replaces the binary atomically. Installing the CLI requires no Rust compiler, Python, or sudo. See [releases and installation](releases.md) for pinned versions and custom destinations.

Windows x64 users can use the [PowerShell installer](releases.md). Install the toolchains for your chosen application target yourself before native verification. Managed native execution supports all five release platforms; macOS and Windows manage the direct child with bounded termination and waiting.

## Build from source

Use current stable Rust through rustup, Cargo, and a C toolchain for bundled SQLite. From this repository's root:

```sh
cargo build --release -p dctx0-cli --locked
export PATH="$PWD/target/release:$PATH"
dctx0 --help
```

Or install from the checkout:

```sh
cargo install --path crates/dctx0-cli --locked
```

All four target features are enabled by default. Parsing, compilation, and task planning require no native application toolchain. Native verification uses the selected runtime/compiler on Linux, macOS or Windows: Bun, Go, or Cargo. Rust also needs a host C toolchain: Xcode Command Line Tools on macOS or an MSVC developer environment with the Windows SDK on Windows. Java profiles do not provide execution.

## Install agent skills

```sh
dctx0 install --project /path/to/app --agent codex,claude
```

This installs skills, not the CLI executable. Other supported agent layouts include `agents`, `copilot`, `cursor`, `gemini`, and `opencode`. Existing differing files are preserved unless explicitly replaced with `--force`.

## Draft your first specification with an agent

After installing the skills, open your coding agent in the application project and paste:

> Use the dctx0-language skill to create dctx0/calculations.dctx0. Specify a pure operation double that takes an integer and returns twice its input. Include an invariant for this rule. Ask me about unresolved product decisions, validate the specification with dctx0 check, and explain the contract in plain language. Wait for my review before implementing the application.

The agent writes the contract and runs the compiler. Review whether the declarations and invariant describe what you meant; successful compilation checks syntax and semantics, not the correctness of your product decisions. You can ask the agent to explain any unfamiliar construct. See [author with your agent](authoring.md) for the full workflow.

## Read the resulting contract

A specification for that request can look like this. You can also create `dctx0/calculations.dctx0` manually if you prefer:

<!-- dctx0:check -->
```dctx0
workspace Calculations {
  domain Numbers {
    module Arithmetic {
      intent "Compute deterministic values from explicit inputs."
      surface Rules {
        operation double(value: Int) -> Int { pure }
      }
      invariant DoublesTheInput {
        forall value: Int
        assert Rules.double(value) == value * 2
      }
    }
    test Properties
  }
}
```

```sh
dctx0 check /path/to/app/dctx0
dctx0 tasks preview /path/to/app/dctx0
dctx0 tasks plan /path/to/app/dctx0
```

The specification compiles before `double` exists. Checking syntax and semantics does not execute the operation or satisfy the invariant against an application.

## Project layout

```text
app/
├── dctx0/                        # reviewed specifications
│   └── calculations.dctx0
├── src/                          # independent application implementation
└── .dctx0/
    ├── profiles/                 # editable implementation guidance
    └── testing/
        ├── verification.json     # target, scope, fixtures, seed, budgets
        └── typescript/           # external adapter and testing artifacts
```

Derived history lives under the spec root's `.dctx0ctl/`. Do not hand-edit SQLite records or generated reports to satisfy a check.

After reviewing the contract, ask your agent to plan the tasks and use the `dctx0-implementation` skill to implement them and connect a testing adapter to the real application. The commands below show the setup that you or the agent can perform.

## Initialize a real adapter

```sh
dctx0 profile typescript create --project /path/to/app --default --agents
dctx0 verify discover /path/to/app/dctx0 --project /path/to/app --target typescript
dctx0 verify init /path/to/app/dctx0 --project /path/to/app --target typescript \
  --subject Calculations.Numbers.Arithmetic
```

Discovery identifies structural candidates, not verified bindings. Initialization creates an unavailable scaffold and preserves existing files. Implement the adapter using actual production entry points. The explicit selection above avoids an empty verification scope.

Then inspect the plan and execute it:

```sh
dctx0 verify plan /path/to/app/dctx0 --project /path/to/app
dctx0 verify run /path/to/app/dctx0 --project /path/to/app --no-adopt
```

Prepare dependencies yourself. A managed native build is not a package-installation command. See [TypeScript adapters](typescript-adapters.md) and [targets](targets.md).

## Try an existing complete application

Cinema already supplies adapters, fixtures, configuration, and authored inspection notes:

```sh
bun install --frozen-lockfile --cwd examples/cinema
dctx0 verify plan examples/cinema/dctx0 --project examples/cinema
dctx0 verify run examples/cinema/dctx0 --project examples/cinema --no-adopt
```

For complete configured acceptance, use `dctx0 verify complete examples/cinema/dctx0 --project examples/cinema`. Current review tasks must have appropriate source-bound reports. A missing, stale, or rejected assigned review prevents acceptance. See [agent reviews](agent-evidence.md).
