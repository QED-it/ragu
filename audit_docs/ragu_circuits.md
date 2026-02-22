## Background for Understanding `ragu_circuits`

### What the crate does

`ragu_circuits` implements the **polynomial machinery** that turns abstract circuit definitions into concrete polynomials for the proof system. It provides the `Circuit` trait, the SXY/RX/SY driver implementations that produce the protocol's key polynomials, the `Mesh` system for multi-circuit registries, and the staging infrastructure for multi-stage witness polynomials.

This is where the abstract driver/gadget model from `ragu_core` meets the polynomial arithmetic from `ragu_arithmetic`.

---

### 1. The Circuit trait

`Circuit<F>` is the user-facing trait for defining arithmetic circuits:

```
trait Circuit<F: Field> {
    type Instance<'source>;   // Public input
    type Witness<'source>;    // Private witness
    type Output;              // Serializable gadget output
    type Aux<'source>;        // Interstitial witness data

    fn instance(...) -> Result<Output>;   // From public inputs
    fn witness(...) -> Result<(Output, Aux)>;  // From witness
}
```

`CircuitExt<F>` extends this with polynomial extraction:
- **`rx()`** — run the circuit under the RX driver to produce the witness polynomial r(X)
- **`ky()`** — compute the public input polynomial k(Y) from the instance
- **`into_object()`** — convert to a `CircuitObject` that can evaluate the wiring polynomial s(X,Y)

See [Writing Circuits](https://tachyon.z.cash/ragu/guide/writing_circuits.html) and [Architecture Overview](https://tachyon.z.cash/ragu/implementation/arch.html).

---

### 2. The SXY driver — wiring polynomial synthesis

The SXY driver (`s/sxy.rs`) evaluates the wiring polynomial s(X, Y) at specific field points (x, y). It is a `Collector` struct that:

- Tracks powers of X for each allocation slot (a, b, c vectors)
- Accumulates contributions from `enforce_zero()` as coefficients of Y^j
- Multiplies by `Coeff` tags from `ragu_arithmetic` for optimization

The wiring polynomial encodes all linear constraints of the circuit:

> s(X, Y) = Σ_{j} Y^j · (Σ_i u_{j,i}·X^{2n-1-i} + Σ_i v_{j,i}·X^{2n+i} + Σ_i w_{j,i}·X^{4n-1-i})
> — [Polynomial Management](https://tachyon.z.cash/ragu/implementation/polynomials.html)

The SXY driver's `MaybeKind = Empty`, so witness closures are never called — the compiler eliminates all witness computation code. This is the "25-30% synthesis cost" optimization mentioned in the [Drivers](https://tachyon.z.cash/ragu/guide/drivers.html) page.

There are two variants:
- **SXY** (`s/sxy.rs`): evaluates s(x, y) → scalar F (point evaluation)
- **SX** (`s/sx.rs`): evaluates s(x, Y) → `Polynomial<F>` (univariate in Y)
- **SY** (`s/sy.rs`): evaluates s(X, y) → `Polynomial<F>` (univariate in X, structured)

---

### 3. The RX driver — witness polynomial generation

The RX driver (`rx.rs`) generates the trace polynomial r(X) by executing the circuit with actual witness values. It uses a `Collector` with `MaybeKind = Always` — all witness closures are called.

The trace polynomial encodes the witness vectors **a**, **b**, **c** as a [structured vector](https://tachyon.z.cash/ragu/protocol/prelim/structured_vectors.html):

> r(X) = Σ_{i} (c_i·X^i + b_i·X^{2n-1-i} + a_i·X^{2n+i})

Wire allocations are packed efficiently: each `mul()` gate produces three values (a, b, a·b) stored in the a, b, c coefficient positions. The `alloc()` operation packs two allocations per gate slot.

See [Drivers](https://tachyon.z.cash/ragu/guide/drivers.html): "the `RX` driver generates witness values... constructing the trace polynomial R(X)."

---

### 4. Polynomial representations

Two polynomial types encode the circuit's polynomial data:

**Structured Polynomial** (`polynomials/structured.rs`):
- Four coefficient vectors: **u**, **v**, **w**, **d** (each of length n)
- Represents: p(X) = Σ_i (c_i·X^i + b_i·X^{2n-1-i} + a_i·X^{2n+i} + d_i·X^{4n-1-i})
- Two views: `Forward` (natural order: u, v, w) and `Backward` (reversed: v, u, d)
- Operations: `eval()`, `dilate()` (p(X) → p(zX)), `revdot()`, `commit()` (via MSM)

**Unstructured Polynomial** (`polynomials/unstructured.rs`):
- Simple monomial basis: coefficient vector [a₀, a₁, ..., a_{4n-1}]
- Can convert to/from structured form
- Operations: `eval()`, `scale()`, `add_structured()`, `commit()`

The structured form mirrors the [structured vector](https://tachyon.z.cash/ragu/protocol/prelim/structured_vectors.html) layout: **c** || rev(**b**) || **a** || **0**. The `revdot()` method computes the revdot product directly from the structured representation.

---

### 5. The Rank system

`Rank` is a compile-time constant (values 2–28) that determines circuit capacity:

- `num_coeffs() = 2^RANK` — total polynomial coefficients (= 4n)
- `n() = 2^(RANK-2)` — maximum multiplication constraints per circuit

The Rank constrains all polynomial sizes throughout the system. When `CircuitExt::into_object()` is called, it validates that the circuit's actual constraint counts fit within the rank.

`Rank` also provides the gate polynomial t(X, Z) computation via `tz()`, `tx()`, `txz()` — these correspond to the [NARK](https://tachyon.z.cash/ragu/protocol/core/nark.html) gate polynomial t(X, Z) = -Σ_i x^{4n-1-i}·(z^{2n-1-i} + z^{2n+i}).

---

### 6. The Mesh — multi-circuit registry

`Mesh<F, R>` implements the [Registry Polynomial](https://tachyon.z.cash/ragu/protocol/extensions/registry.html) m(W, X, Y) that interpolates all registered circuit wiring polynomials:

> m(ω^i, X, Y) = s_i(X, Y)

Key operations:
- **`wxy(w, x, y)`** — full evaluation m(w, x, y) ∈ F
- **`wy(w, y)`** — restriction m(w, X, y), returns a polynomial in X
- **`wx(w, x)`** — restriction m(w, x, Y), returns a polynomial in Y
- **`xy(x, y)`** — restriction m(W, x, y), returns a polynomial in W

`CircuitIndex` maps circuit registration order to domain points using bit-reversal: `bitreverse(j, S)` → ω_S^i. This enables incremental registration without knowing the final registry size (see [Registry — Bit-Reversal](https://tachyon.z.cash/ragu/protocol/extensions/registry.html)).

`MeshBuilder` registers circuits and finalizes with a key = H(m(w, x, y)) digest for non-trivial evaluations.

The mesh evaluations use `Domain::ell()` (barycentric Lagrange interpolation from `ragu_arithmetic`) to interpolate across the domain of roots of unity.

---

### 7. Staging

The [staging](https://tachyon.z.cash/ragu/protocol/extensions/staging.html) system allows circuits to compute their witness polynomial in multiple stages rather than all at once:

> r(X) = r'(X) + a(X) + b(X) + ...

**`Stage<F, R>`** — represents one partial witness polynomial. Defines how many wires it allocates and how to compute its portion of the witness.

**`StagedCircuit<F, R>`** — like `Circuit` but witness takes a `StageBuilder` that allocates stages sequentially.

**`StageBuilder`** — orchestrates stage construction:
- `add_stage()` → allocates wire positions, returns `StageGuard`
- `StageGuard::enforced()` — computes stage witness, constrains equality with allocated wires
- `StageGuard::unenforced()` — injects stage wires without constraint (used for preamble)
- `skip_stage()` — reserves positions without computing

**`StageObject`** — generates the well-formedness constraint polynomial for a stage, ensuring multiplication gates in the stage's range are correctly formed.

Staging connects to the [nested staged commitments](https://tachyon.z.cash/ragu/protocol/core/accumulation/pcs.html) used to achieve input consistency across the curve cycle without non-native arithmetic.

---

### 8. Metrics

The `metrics` module provides constraint counting:
- `Counter` — lightweight driver that counts constraints without computing values
- `CircuitMetrics` — holds num_multiplication, num_linear, degree_ky
- Used by `CircuitExt::into_object()` to validate circuits fit within rank bounds

---

### How it maps to the protocol

```
Wiring polynomial s(X, Y)             ← SXY/SX/SY drivers
Trace polynomial r(X)                 ← RX driver
Public input polynomial k(Y)          ← CircuitExt::ky()
Gate polynomial t(X, Z)               ← Rank::txz()
Structured vector r = c||b̂||a||0      ← Structured polynomial
Registry polynomial m(W, X, Y)        ← Mesh + Domain::ell()
Bit-reversal circuit indexing          ← CircuitIndex + bitreverse()
Multi-stage witness decomposition      ← Stage + StageBuilder
Stage well-formedness checks           ← StageObject
Polynomial commitment (Pedersen)       ← Polynomial::commit() → MSM
Circuit constraint validation          ← Metrics + Rank bounds
```

### Reading order

1. **`src/lib.rs`** — Circuit trait, CircuitExt, CircuitObject
2. **`src/polynomials/structured.rs`** — Structured polynomial (the core data type)
3. **`src/polynomials/unstructured.rs`** — Unstructured polynomial
4. **`src/s/sxy.rs`** — SXY driver (point evaluation of s)
5. **`src/rx.rs`** — RX driver (witness polynomial generation)
6. **`src/s/sy.rs`** — SY driver (restriction s(X, y))
7. **`src/s/sx.rs`** — SX driver (restriction s(x, Y))
8. **`src/polynomials/txz.rs`** — Gate polynomial t(X, Z)
9. **`src/mesh.rs`** — Mesh / registry polynomial
10. **`src/staging/mod.rs`** — Stage, StagedCircuit, StageBuilder
11. **`src/metrics.rs`** — Constraint counting
12. **`src/polynomials/rank.rs`** — Rank system
