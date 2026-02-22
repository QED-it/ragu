## Background for Understanding `ragu_primitives`

### What the crate does

`ragu_primitives` provides the concrete **gadget implementations** that circuits are built from: Boolean, Element (field element), Point (elliptic curve point), and Endoscalar. It also provides the Poseidon sponge hash, the Simulator driver for testing, and the IO/serialization infrastructure for encoding gadgets into polynomials.

This is the layer where abstract driver operations meet concrete cryptographic types.

---

### 1. Element gadget

`Element<'dr, D>` wraps a single wire and an optional witness value (`DriverValue<D, F>`). It is the fundamental building block — every other gadget is composed of Elements.

Arithmetic operations map directly to driver calls:
- **`mul()`** → `driver.mul()` — costs one multiplication constraint
- **`add()`, `sub()`, `scale()`, `double()`** → `driver.add()` — free (virtual wire via linear combination)
- **`square()`** → `driver.mul()` with a = b — one constraint
- **`invert()`** → allocates inverse as witness, constrains x · x⁻¹ = 1 — one constraint
- **`is_zero()`** → inverse trick: allocate inv as witness, constrain `x · inv = 1 - b` and `x · b = 0` where b is boolean — two constraints

The `enforce_zero()` method constrains an element to equal zero via `driver.enforce_zero()`, creating one [linear constraint](https://tachyon.z.cash/ragu/protocol/core/arithmetization.html).

---

### 2. Boolean gadget

`Boolean<'dr, D>` wraps a wire constrained to be 0 or 1. The constraint is `b · (1 - b) = 0`, which costs one multiplication gate.

Key operations:
- **`and(a, b)`** → `a · b` — one multiplication constraint
- **`not()`** → `1 - b` — free (linear combination)
- **`conditional_select(flag, a, b)`** → `flag · a + (1 - flag) · b` — one constraint
- **`conditional_enforce_equal(flag, a, b)`** → `flag · (a - b) = 0` — one constraint

Booleans are used pervasively for bit decompositions, conditional logic, and the `multipack()` function that packs boolean slices into field elements by field capacity.

---

### 3. Point gadget

`Point<'dr, D, C>` represents an affine point (x, y) on an elliptic curve C, stored as two Element wires. The curve C must implement `CurveAffine` (from `ragu_arithmetic`).

Elliptic curve operations in-circuit:
- **`double()`** — point doubling using the tangent line formula. Costs: 1 inversion + several multiplications.
- **`add_incomplete()`** — point addition assuming P ≠ ±Q and neither is identity. Uses the chord formula.
- **`double_and_add_incomplete()`** — combined double-and-add for efficiency.
- **`negate()`** — negate y-coordinate (free, linear combination).
- **`endo()`** — apply the curve endomorphism to the x-coordinate. For Pasta curves, this multiplies x by a specific cube root of unity ζ. Costs one multiplication.

These operations are "incomplete" — they don't handle edge cases (identity point, equal points for addition). This is safe in the protocol because the verifier challenges ensure points are random and distinct with overwhelming probability.

Point operations are central to the [Bulletproofs IPA](https://tachyon.z.cash/ragu/protocol/prelim/bulletproofs.html) verification in-circuit and the [nested commitment](https://tachyon.z.cash/ragu/protocol/prelim/nested_commitment.html) scheme.

---

### 4. Endoscalar gadget

`Endoscalar<'dr, D>` represents a cross-circuit scalar — a λ-bit binary string (128 or 136 bits) that can be efficiently mapped to both fields in the curve cycle.

Key operations:
- **`extract()`** — given a field element, extract the endoscalar bits. Constrains the extraction via bit decomposition in-circuit.
- **`field_scale()`** — multiply a field element by the endoscalar (native scalar arithmetic).
- **`group_scale()`** — multiply a curve point by the endoscalar using the efficient endomorphism (endoscaling).

The book describes endoscalars as the bridge for moving challenges across the curve cycle:

> "First, run extract in the Fp-circuit to obtain the endoscalar as a public output; then use the same endoscalar as the public input of the Fq-circuit and constrain endo(s)·G completely natively."
> — [Endoscalars](https://tachyon.z.cash/ragu/protocol/extensions/endoscalar.html)

The helper functions `compute_endoscalar()` and `extract_endoscalar()` provide the out-of-circuit (native) versions of these operations.

---

### 5. Poseidon sponge

`Sponge<'dr, D, P>` implements the [Poseidon](https://tachyon.z.cash/ragu/protocol/prelim/assumptions.html) algebraic hash function as a sponge construction over the driver.

The sponge has two phases:
- **Absorb** — feed field elements into the rate portion of the state
- **Squeeze** — extract field elements from the rate portion after permuting

Each permutation applies the Poseidon round function: full rounds (all S-boxes active) alternating with partial rounds (one S-box active), with MDS matrix mixing between rounds. The permutation parameters come from `P: PoseidonPermutation<F>` (defined in `ragu_arithmetic`, instantiated in `ragu_pasta`).

`SpongeState<'dr, D, P>` holds the raw T-element state as a `FixedVec<Element, PoseidonStateLen>`.

The sponge is used for Fiat-Shamir challenge generation throughout the [protocol transcript](https://tachyon.z.cash/ragu/protocol/prelim/transcript.html). Both in-circuit (for the recursive verifier) and out-of-circuit (for the prover).

---

### 6. Simulator driver

`Simulator<F>` is a testing driver that **fully simulates** constraint synthesis and **validates** all constraints:

- `mul()` — allocates (a, b, c) and checks a · b = c
- `enforce_zero()` — evaluates the linear combination and asserts it equals zero
- `alloc()` — calls the witness closure and stores the value

It tracks metrics:
- `num_allocations()` — total wire count
- `num_multiplications()` — multiplication constraint count
- `num_linear_constraints()` — linear constraint count

The Simulator catches bugs that the Emulator would miss (since the Emulator doesn't enforce constraints). It's the primary tool for testing circuit correctness.

See [Drivers](https://tachyon.z.cash/ragu/guide/drivers.html): "the `Simulator` driver fully simulates synthesis and validates constraints for testing purposes."

---

### 7. FixedVec and compile-time lengths

`FixedVec<T, L>` is a vector with a compile-time guaranteed length, parameterized by `L: Len`. The `ConstLen<N>` type provides const-generic length markers.

This exists because gadgets must be **fungible** — their wire count must be deterministic. Standard `Vec<T>` has dynamic length, which would break the synthesis invariants. `FixedVec` derefs to `[T]` and implements `Gadget` and `Write`.

The `CollectFixed` trait provides `.collect_fixed()` for iterators.

---

### 8. IO: Write trait and Buffer

The `Write<F>` trait serializes gadgets into a `Buffer` of `Element` values. This is how gadget outputs become part of the public input polynomial k(Y) — each element is written sequentially, and the buffer contents map to coefficients of k(Y).

`Buffer<'dr, D>` is the destination trait, implemented for:
- `Vec<Element>` — collects elements
- `()` — no-op (for counting or when output isn't needed)
- `usize` — counts elements without storing

The `Pipe` struct enables cross-driver element transfer.

The `Write` trait connects to [Polynomial Management](https://tachyon.z.cash/ragu/implementation/polynomials.html) where the public input polynomial is constructed from circuit outputs.

---

### 9. Promotion and Demoted

`Promotion<F>` enables stripping witness data from a gadget (`Demoted<'dr, D, G>`) and later restoring it with new witness values (`promote()`). This is used when a gadget's structure (wires) needs to persist across synthesis passes but witness values should be recomputed.

---

### How it maps to the protocol

```
Multiplication constraints (a·b = c)       ← Element::mul(), Boolean::and()
Linear constraints (wiring)                ← Element::enforce_zero(), add/sub/scale
In-circuit curve operations                ← Point::add_incomplete(), double()
Cross-circuit challenge transport          ← Endoscalar::extract(), group_scale()
Fiat-Shamir transcript hashing             ← Sponge::absorb(), squeeze()
Public input polynomial k(Y)              ← Write trait serialization
Constraint validation during testing       ← Simulator
Deterministic gadget sizing                ← FixedVec<T, L>
Nested commitment coordinate encoding      ← Point gadget (x, y as Elements)
Endomorphism-based scalar multiplication   ← Point::endo(), Endoscalar::group_scale()
```

### Reading order

1. **`src/element.rs`** — Element gadget (foundation for everything)
2. **`src/boolean.rs`** — Boolean gadget and bit operations
3. **`src/point.rs`** — Elliptic curve point gadget
4. **`src/endoscalar.rs`** — Endoscalar cross-circuit bridge
5. **`src/poseidon.rs`** — Sponge hash (Poseidon permutation)
6. **`src/simulator.rs`** — Simulator driver for testing
7. **`src/vec.rs`** — FixedVec compile-time length vectors
8. **`src/io.rs`** — Write/Buffer traits
9. **`src/io/pipe.rs`** — Cross-driver element transfer
10. **`src/promotion.rs`** — Demoted/Promotion witness stripping
11. **`src/util.rs`** — Maybe extensions
12. **`src/foreign.rs`** — Write impls for phantom types
