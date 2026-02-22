## Background for Understanding `ragu_pcd`

### What the crate does

`ragu_pcd` is the **top-level crate** — it implements the full proof-carrying data (PCD) pipeline. It orchestrates everything below it: circuits, polynomials, commitments, accumulation, and recursion. It provides the `Step` trait for defining computational nodes, the `Application` builder for registering circuits, and the 11-stage fuse pipeline that produces and verifies proofs.

This is the largest crate (~59 files) and the most protocol-dense.

---

### 1. Proof-carrying data (PCD)

PCD is a generalization of incrementally-verifiable computation (IVC) to tree structures. Each node in the tree:
- Receives proofs from its two children (left, right)
- Executes a transition function (the "step")
- Produces a new proof that attests to the correctness of the entire subtree

Ragu implements **2-arity PCD** — each step takes two input proofs. The base case uses "trivial" proofs (no children). See [Proof-carrying data](https://tachyon.z.cash/ragu/concepts/pcd.html).

---

### 2. The Step trait

`Step<C: Cycle>` defines a computational node in the PCD tree:

- `INDEX: usize` — unique circuit identifier (used for registry indexing)
- `Left`, `Right` — `Header` types for child proofs
- `Output` — `Header` type for the produced proof
- `Witness<'source>` — private witness data
- `Aux<'source>` — auxiliary data from proof pipeline

The `witness()` method synthesizes the step using a driver, consuming left/right header gadgets and producing the output header gadget.

See [Writing Circuits](https://tachyon.z.cash/ragu/guide/writing_circuits.html) for how users define steps.

---

### 3. The Header trait

`Header<F>` represents the publicly-visible state carried by a proof:

- `SUFFIX: Suffix` — unique type identifier (used for hashing)
- `Data<'source>` — witness representation
- `Output` — gadget form implementing `Write`

The `encode()` method converts raw data into a gadget under a driver. Headers are serialized into the public input polynomial k(Y) via the `Write` trait.

Two built-in headers:
- `()` — trivial header (no data), used for base cases
- Application-defined headers — carry computation state through the PCD tree

---

### 4. Application builder

`ApplicationBuilder<'params, C, R, HEADER_SIZE>` is the entry point for constructing a PCD application:

1. **`register::<S: Step>()`** — register application steps sequentially, each gets a `CircuitIndex`
2. **`finalize()`** — builds native and nested meshes ([registry polynomials](https://tachyon.z.cash/ragu/protocol/extensions/registry.html)), registers internal steps (trivial, rerandomization), produces an `Application`

The `Application` provides three operations:
- **`seed()`** — create a leaf proof (no children, trivial inputs)
- **`fuse()`** — combine two proofs using a step (the main recursive operation)
- **`rerandomize()`** — privacy-preserving proof transformation

---

### 5. The Proof structure

`Proof<C, R>` contains all polynomial data needed for verification:

| Component | Protocol role |
|---|---|
| `application` | Application step output + r(X) polynomial |
| `preamble` | Preamble stage polynomial (input encoding) |
| `s_prime` | Modified wiring polynomial commitment |
| `error_n`, `error_m` | Error polynomials for accumulation |
| `ab` | Polynomial pair (a, b) + combined value c = revdot(a, b) |
| `query` | Query polynomial for mesh evaluation |
| `f` | Fused polynomial combining all components |
| `eval` | Evaluation polynomial for PCS |
| `p` | Final polynomial with evaluation p(u) = v |
| `challenges` | All verifier challenges (w, y, z, μ, ν, x, α, u, β) |
| `circuits` | Internal circuit polynomials for recursion |

`Pcd<'source, C, R, H>` wraps a `Proof` with typed header data.

---

### 6. The fuse pipeline (11 stages)

The `fuse()` operation runs an 11-stage pipeline that implements the full [NARK](https://tachyon.z.cash/ragu/protocol/core/nark.html) protocol:

**Stage _01 — Application**: Runs the user's step circuit. Produces left/right encodings and application proof components.

**Stage _02 — Preamble**: Generates the [preamble stage](https://tachyon.z.cash/ragu/protocol/core/accumulation/pcs.html) polynomial for both left and right proofs. This encodes the input instances for cross-circuit consistency via [nested staged commitments](https://tachyon.z.cash/ragu/protocol/core/accumulation/pcs.html).

**Stage _03 — S'**: Computes the modified wiring polynomial s'(X, y). Absorbs its commitment into the transcript, then squeezes challenges y and z.

**Stage _04, _05 — Error_M, Error_N**: Compute the error polynomials that capture the difference between the left and right accumulator claims. These correspond to the off-diagonal entries in the [revdot folding](https://tachyon.z.cash/ragu/protocol/core/accumulation/revdot.html) error matrix. Squeezes challenges μ, ν, μ', ν'.

**Stage _06 — AB**: Computes the folded polynomial pair (a, b) from the error components and previous accumulators. The combined value c = revdot(a, b) is the [consolidated constraint](https://tachyon.z.cash/ragu/protocol/core/nark.html) check. Squeezes challenge x.

**Stage _07 — Query**: Constructs the query polynomial for [mesh/registry evaluation](https://tachyon.z.cash/ragu/protocol/extensions/registry.html) consistency check.

**Stage _08 — F**: Builds the fused polynomial combining all polynomial oracle commitments. Implements the [PCS batched evaluation](https://tachyon.z.cash/ragu/protocol/core/accumulation/pcs.html) — quotient polynomials q_i(X) = (p_i(X) - y_i)/(X - x_i) are linearly combined with challenge α. Squeezes challenges α and u.

**Stage _09 — Eval**: Evaluates all polynomials at the challenge point u, sending p_i(u) values to the verifier.

**Stage _10 — P**: Final polynomial aggregation p(X) = f(X) + Σ βⁱ·p_i(X) with explicit evaluation p(u) = v. Squeezes pre_β challenge.

**Stage _11 — Circuits**: Runs internal recursive circuits (rerandomization, trivial) to produce the circuit polynomials needed by the next recursion step.

---

### 7. Two-field recursion

Ragu operates over a [curve cycle](https://tachyon.z.cash/ragu/protocol/index.html#cycles), so the proof pipeline maintains polynomial data on both fields:

- **Native** (Fp / CircuitField): wiring polynomial s(X,Y), trace r(X), public input k(Y)
- **Nested** (Fq / ScalarField): the same polynomial types for the secondary curve

The `Application` holds two meshes — one per field. The fuse pipeline processes both native and nested components in each stage.

This two-field structure implements the [split accumulation](https://tachyon.z.cash/ragu/protocol/core/accumulation/pcs.html) design:

> "Evaluations are folded in their field-native circuit while commitments are folded in the other merge circuit where group arithmetic is native."
> — [PCS — Ragu Adaptation](https://tachyon.z.cash/ragu/protocol/core/accumulation/pcs.html)

---

### 8. Revdot claims and folding

The `components/claims/` module builds and manages [revdot product](https://tachyon.z.cash/ragu/protocol/core/accumulation/revdot.html) claims — the core verification primitive:

> revdot(**a**, **b**) = k(y)

Each proof carries accumulated revdot claims from all previous proofs in the tree. During fuse, new claims are folded into the accumulator using random challenges (μ, ν). The error matrix captures cross-terms between claims.

The `components/fold_revdot/` module implements the two-layer reduction scheme that reduces constraint count from (MN)² to NM² + N² - N + 3.

---

### 9. Verification

`Application::verify()` checks a proof by:

1. Verifying the circuit ID is in the mesh domain
2. Checking header sizes match HEADER_SIZE
3. Computing k(y) values from public inputs
4. Checking all native revdot claims: revdot(a, b_reversed) = k(y)
5. Checking all nested revdot claims
6. Verifying polynomial evaluation: p(u) = v
7. Verifying the P commitment
8. Checking mesh polynomial evaluation: m(w, x, y) matches stored value

This corresponds to the [NARK verification checks](https://tachyon.z.cash/ragu/protocol/core/nark.html): correct decomposition, consolidated CS check, and public "one" constraint.

---

### 10. Internal steps

Two built-in steps handle protocol infrastructure:

- **Trivial** (index 1) — produces empty proofs for PCD tree leaves. The trivial circuit has zero constraints.
- **Rerandomize** (index 0) — transforms a proof to break the link between prover identity and proof content, enabling privacy in the PCD tree. See [PCD — Rerandomization](https://tachyon.z.cash/ragu/concepts/pcd.html).

---

### 11. Components

Reusable building blocks in `components/`:

| Component | Purpose |
|---|---|
| `claims/` | Revdot claim construction from r(X) polynomials |
| `endoscalar/` | Endoscalar operations for cross-circuit challenges |
| `fold_revdot/` | Two-layer revdot folding with error matrix |
| `horner/` | Horner evaluation as a circuit gadget |
| `ky/` | Public input polynomial k(Y) computation |
| `root_of_unity/` | Root of unity ω for domain operations |
| `suffix/` | Suffix encoding for header type identification |

---

### How it maps to the protocol

```
PCD tree (2-arity)                      ← Step trait, seed/fuse/rerandomize
NARK protocol (9 steps)                 ← Fuse stages _01–_10
Registry polynomial m(W, X, Y)          ← Application meshes + Stage _07
Trace polynomial r(X)                   ← Stage _01 (RX driver)
Consolidated constraint revdot=k(y)     ← Stage _06 (AB)
PCS batched evaluation (quotients)      ← Stage _08 (F) using factor()
Accumulation (revdot folding)           ← Stages _04/_05 (error polynomials)
Split accumulation (two-field)          ← Native + nested polynomial pairs
Nested staged commitments               ← Stage _02 (preamble)
Endoscalar challenge transport          ← components/endoscalar/
Fiat-Shamir transcript                  ← Poseidon sponge throughout
Final verification                      ← verify() method
Recursive circuit polynomials           ← Stage _11 (circuits)
```

### Reading order

1. **`src/lib.rs`** — module structure and re-exports
2. **`src/step/mod.rs`** — Step trait, InternalStepIndex
3. **`src/header.rs`** — Header trait, Suffix
4. **`src/application.rs`** — ApplicationBuilder, Application (seed/fuse/verify)
5. **`src/proof/mod.rs`** — Proof structure and components
6. **`src/fuse/mod.rs`** — Fuse pipeline orchestration
7. **`src/fuse/_01_application.rs`** through **`_11_circuits.rs`** — individual stages
8. **`src/verify.rs`** — Verification logic
9. **`src/components/claims/`** — Revdot claim building
10. **`src/components/fold_revdot/`** — Revdot folding
11. **`src/circuits/native/`** — Native recursive circuits
12. **`src/circuits/nested/`** — Nested recursive circuits
13. **`src/step/internal/`** — Trivial, rerandomize, adapter
