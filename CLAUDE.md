# CLAUDE.md — Ragu Project Guide

## What is Ragu?

Ragu is a Rust proof-carrying data (PCD) framework implementing a modified Halo-based recursive SNARK construction. It targets the Pasta curves (Pallas/Vesta) used in Zcash, designed for Project Tachyon. No trusted setup required.

**Status**: Under heavy development, version 0.0.0, not audited. Author: Sean Bowe.

**Repository**: `tachyon-zcash/ragu` — License: MIT/Apache-2.0

## Key Concepts

- **PCD (Proof-Carrying Data)**: Data bundled with a proof of correctness; proofs can take prior proofs as input, enabling recursive/incremental verification. Ragu treats every step as arity-2 (two proof inputs).
- **IVC**: Linear-chain special case of PCD.
- **Accumulation/Folding**: Technique from Halo where full verification is continually collapsed, avoiding expensive re-verification at each step.
- **Driver**: Compile-time specialized backend interpreter for circuit code. Same circuit code works across synthesis (SXY), witness generation (RX), emulation (Emulator), and simulation (Simulator) contexts.
- **Gadget**: Structural unit of arithmetic circuits — wires + witness data + constraints. Must be fungible, thread-safe, `'static`, and `Clone`.
- **Routine**: Reusable circuit transformation with `predict()`/`execute()` enabling memoization and parallelization.
- **Circuit**: Trait with Instance, Witness, Output, and Aux associated types.
- **Mesh**: Structure interpolating multiple circuit polynomials for non-uniform circuit support.
- **Step**: Application-defined PCD computation step.

## Crates

| Crate | Files | Description |
|-------|-------|-------------|
| **ragu_macros** | 11 | Proc macros: `#[derive(Gadget)]`, `#[derive(Write)]`, `repr256!`, `gadget_kind!` |
| **ragu_arithmetic** | 6 | Math foundations: `Cycle` trait, `Domain` (FFT), Poseidon specs, field utilities |
| **ragu_core** | 13 | Core abstractions: `Driver`, `Gadget`, `GadgetKind`, `Maybe<T>` monad, `Routine` |
| **ragu_primitives** | 13 | Primitive gadgets: `Element`, `Boolean`, `Point`, `Endoscalar`, Poseidon `Sponge`, `FixedVec` |
| **ragu_circuits** | 18 | Circuit synthesis: `Circuit` trait, `Mesh` (multi-circuit polynomials), S(X,Y) encoding, staging |
| **ragu_pasta** | 4 | Pasta curve `Cycle` impl: Pallas/Vesta generators, Poseidon params, `baked` feature for compile-time constants |
| **ragu_gadgets** | 1 | Placeholder stub (empty, may be removed) |
| **ragu_pcd** | 59 | PCD system: `Step`, `Proof`, `Application`, 11-stage fuse pipeline, native/nested circuits, verification |

## Project Structure

```
ragu/                          # Root crate — primary user-facing API (re-exports sub-crates)
├── book/                      # mdbook documentation ("The Ragu Book")
│   └── src/
│       ├── guide/             # User guide (circuits, gadgets, drivers, routines)
│       ├── protocol/          # Protocol design (arithmetization, NARK, accumulation)
│       ├── implementation/    # Architecture, circuits, polynomials, drivers
│       └── appendix/          # Related work, terminology
├── crates/
│   ├── ragu_arithmetic/       # Math traits: Cycle, Domain, FFT, Poseidon specs (6 files)
│   ├── ragu_core/             # Fundamental traits: Driver, Gadget, Maybe, Routine (13 files)
│   ├── ragu_macros/           # Proc macros: #[derive(Gadget)], #[derive(Write)], repr256, gadget_kind (11 files)
│   ├── ragu_primitives/       # Primitive gadgets: Element, Boolean, Point, Endoscalar, Poseidon sponge (13 files)
│   ├── ragu_circuits/         # Circuit synthesis: Circuit trait, Mesh, polynomial management, staging (18 files)
│   ├── ragu_pasta/            # Pasta curve Cycle impl: Pallas/Vesta generators, Poseidon params (4 files + build.rs)
│   ├── ragu_pcd/              # PCD system: Step, Proof, Application, fuse pipeline, verify (59 files)
│   └── ragu_gadgets/          # Placeholder/stub (empty)
├── qa/                        # QA scripts for book (broken links, dead pages)
├── src/lib.rs                 # Root crate lib (no_std, no public API yet)
├── justfile                   # Task runner commands
└── .github/workflows/         # CI: fmt, clippy, test, coverage, book build
```

## Crate Dependency Layering

```
ragu_macros          (no internal deps)
│
├── ragu_arithmetic  (← ragu_macros)
│   │
│   ├── ragu_core    (← ragu_arithmetic, ragu_macros)
│   │   │
│   │   ├── ragu_primitives  (← ragu_arithmetic, ragu_core, ragu_macros)
│   │   │   │
│   │   │   └── ragu_circuits  (← ragu_arithmetic, ragu_core, ragu_primitives)
│   │   │       │
│   │   │       └── ragu_pcd  (← ragu_arithmetic, ragu_core, ragu_primitives, ragu_circuits)
│   │   │           │
│   │   │           └── ragu (root)
│   │   │
│   │   └── ragu_circuits
│   │
│   └── ragu_pasta   (← ragu_arithmetic)  [dev-dependencies only]
│
└── ragu_gadgets     (no deps — empty stub)
```

## Build & Development

- **Rust edition**: 2024, MSRV: 1.90.0 (pinned in `rust-toolchain.toml`)
- **`no_std`** by default at root crate level
- **Build**: `cargo build` or `just build`
- **Test**: `cargo test --release --all --locked`
- **Lint**: `just lint` (clippy + fmt + typos + mdbook build)
- **Fix**: `just fix` (auto-format + clippy fix + typos fix)
- **Book**: `just book serve` (requires mdbook + plugins installed via `just _book_setup`)
- **Full CI locally**: `just ci_local`

### CI Pipeline (`.github/workflows/rust.yml`)

Runs on push to main and PRs. Change detection filters rust vs book changes:
- `fmt` — rustfmt check
- `clippy` — workspace lints, `-D warnings`
- `test` — release tests on ubuntu (32-bit and 64-bit)
- `coverage` — llvm-cov → Codecov
- `bitrot` — build benches/examples with all features
- `docs` — intra-doc link check with `-D warnings`
- `book` — mdbook build

### Book Plugins

mdbook 0.4.52, mdbook-katex (LaTeX math), mdbook-mermaid (diagrams), mdbook-admonish (callouts), mdbook-linkcheck

## Key Types Quick Reference

| Type | Crate | Purpose |
|------|-------|---------|
| `Cycle` | ragu_arithmetic | Marker trait for elliptic curve cycle (Pallas/Vesta) |
| `Domain<F>` | ragu_arithmetic | Radix-2 FFT evaluation domain |
| `Driver<'dr>` | ragu_core | Backend interpreter for circuit execution |
| `Gadget<'dr, D>` | ragu_core | Trait for circuit variable abstractions |
| `GadgetKind<F>` | ragu_core | Unsafe trait relating gadgets across drivers |
| `Maybe<T>` | ragu_core | Type-level optional witness data |
| `Routine<F>` | ragu_core | Reusable circuit section with predict/execute |
| `Element<'dr, D>` | ragu_primitives | Wire + witness field element gadget |
| `Boolean<'dr, D>` | ragu_primitives | Constrained 0/1 wire gadget |
| `Point<'dr, D, C>` | ragu_primitives | Affine curve point gadget |
| `Sponge<'dr, D, P>` | ragu_primitives | Poseidon sponge hash |
| `Circuit<F>` | ragu_circuits | Trait for defining arithmetic circuits |
| `Mesh<F, R>` | ragu_circuits | Multi-circuit polynomial interpolation |
| `Step<C>` | ragu_pcd | Application-defined PCD computation step |
| `Proof<C, R>` | ragu_pcd | Recursive proof structure |
| `Application<..>` | ragu_pcd | Finalized PCD application (meshes + params) |
| `Pasta` | ragu_pasta | Cycle implementation for Pasta curves |

## Protocol Design Summary

- **Arithmetization**: Simple R1CS-like system from BCCGP16 lineage. Witness vectors a, b, c with multiplication + linear constraints consolidated via random challenges into a single revdot equation.
- **Polynomial Commitments**: Pedersen vector commitments + modified Bulletproofs IPA, with SHPLONK multi-point queries.
- **Accumulation**: Split-accumulation (Halo-style) for PCS batched evaluation, wiring consistency, and revdot products.
- **Recursion**: CycleFold-inspired design eliminating unnecessary non-native field arithmetic. Post-processing (no pre-processing/verification keys needed).
- **Non-uniform PCD**: Supports hundreds/thousands of circuits switching freely within the computation graph.

## Conventions

- Procedural macro attributes: `#[ragu(wire)]`, `#[ragu(value)]`, `#[ragu(gadget)]`, `#[ragu(phantom)]`, `#[ragu(driver)]`, `#[ragu(skip)]`
- Typo checker config in `_typos.toml` — extends exclusions for known terms (ND, Bootle)
- KaTeX math rendering in docs via `katex-header.html`
- Workspace dependencies centralized in root `Cargo.toml`

## MSRV Update Checklist

When bumping minimum supported Rust version, update all of:
1. `rust-toolchain.toml` → `toolchain.channel`
2. `.github/actions/rust-setup/action.yml`
3. `book/src/guide/requirements.md`
4. Root `Cargo.toml` → `workspace.package.rust-version`
