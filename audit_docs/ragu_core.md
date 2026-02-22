## Background for Understanding `ragu_core`

### What the crate does

`ragu_core` defines the **central abstractions** of Ragu's circuit programming model: the `Driver` trait, the `Maybe<T>` monad, the `Gadget`/`GadgetKind` traits, and the `Routine` trait. Every circuit in the system is written generically over these abstractions, enabling the same code to run under different backends (synthesis, witness generation, simulation, emulation).

This crate contains no cryptographic math — it is pure abstraction infrastructure.

---

### 1. The Driver trait

The `Driver<'dr>` trait is the core interface that all circuit code is written against. It provides operations to build arithmetic circuits:

- **`alloc()`** — allocate a new wire, optionally with a witness value
- **`mul()`** — allocate three wires (a, b, c) constrained by a·b = c (multiplication gate)
- **`add()`** — create a virtual wire as a linear combination of existing wires (free, unlimited fan-in)
- **`enforce_zero()`** — constrain a linear combination of wires to equal zero (linear constraint)
- **`constant()`** — create a wire with a fixed known value

Different driver implementations interpret these operations differently. The book describes this in detail:

> "Circuits are written generically over the `Driver` trait. They invoke generic driver operations like `driver.mul()` and `driver.enforce_zero()` that each driver interprets differently."
> — [Drivers](https://tachyon.z.cash/ragu/guide/drivers.html)

The `DriverTypes` trait separates the associated types:
- `ImplField: Field` — the field F
- `ImplWire: Clone` — what a "wire" is (field element, polynomial variable, unit, etc.)
- `MaybeKind` — whether witness values exist (`Always` or `Empty`)
- `LCadd` / `LCenforce` — linear expression types for `add()` and `enforce_zero()`

These map directly to the [Bootle16 arithmetization](https://tachyon.z.cash/ragu/protocol/core/arithmetization.html): `mul()` creates multiplication constraints (a_i · b_i = c_i), and `enforce_zero()` creates linear constraints over the witness vectors **a**, **b**, **c**.

---

### 2. The Maybe monad

The `Maybe<T>` trait and `MaybeKind` higher-kinded type provide **type-level optionality** for witness data. This is Ragu's key optimization for separating synthesis from witness generation:

- **`Always<T>`** — wrapper around T. Closures are always called, witness values exist. Used by the RX driver (witness generation) and Simulator.
- **`Empty`** — zero-sized type. Closures are never called and get dead-code eliminated by the compiler. Used by the SXY driver (constraint synthesis).

`DriverValue<D, T>` is the type alias `<<D as DriverTypes>::MaybeKind as MaybeKind>::Rebind<T>`, resolving to either `Always<T>` or `Empty` depending on the driver.

Key operations:
- `just(f)` — wrap a value (calls f if Always, no-ops if Empty)
- `take()` — unwrap (returns T if Always, panics-but-dead-code-eliminated if Empty)
- `cast()` — distribute: `Maybe<(A, B)>` → `(Maybe<A>, Maybe<B>)` via `MaybeCast`
- `map()`, `and_then()` — monadic operations

The book explains the motivation:

> "In some frameworks, circuit synthesis alone accounts for 25-30% of the proof generation time... Ragu maintains separation of concerns through the `Maybe<T>` abstraction."
> — [Drivers](https://tachyon.z.cash/ragu/guide/drivers.html)

---

### 3. The Gadget and GadgetKind traits

A **gadget** is a bundle of wires and optional witness data that represents a circuit-level value (a boolean, a field element, an elliptic curve point, etc.).

`Gadget<'dr, D>` requires:
- **`Clone`** — gadgets must be cloneable (fungibility)
- **`map()`** — convert wires from one driver to another via `FromDriver`
- **`enforce_equal()`** — constrain two gadgets to have equal wire values
- **`num_wires()`** — count of wires for serialization

`GadgetKind<F>` is the driver-agnostic identity. It uses a GAT `Rebind<'dr, D>` to recover the concrete type for any driver. This is how the same gadget definition works across SXY, RX, Emulator, and Simulator backends.

Fungibility rules (from the book):
- No dynamic-length collections (use `FixedVec`)
- No enum discriminants
- Must be `Send + 'static`

See [Gadgets](https://tachyon.z.cash/ragu/guide/gadgets/index.html), [Simple Gadgets](https://tachyon.z.cash/ragu/guide/gadgets/simple.html), and [The GadgetKind Trait](https://tachyon.z.cash/ragu/guide/gadgets/gadgetkind.html).

---

### 4. The LinearExpression trait

`LinearExpression<W, F>` models a linear combination of wires: c₁·w₁ + c₂·w₂ + ... where coefficients are `Coeff<F>` values (from `ragu_arithmetic`).

Operations:
- `add_term(wire, coeff)` — append c·w
- `gain(coeff)` — scale all subsequent terms
- `add(wire)` / `sub(wire)` — shorthand for coeff=1 / coeff=-1
- `extend(iter)` — bulk append

The `DirectSum<F>` implementation accumulates the scalar value directly (used when wires are field elements, as in the Emulator). It leverages `Coeff` tag optimizations — multiplying by `Coeff::Zero` skips the operation entirely.

Linear expressions appear in two driver operations:
- `add()` — creates a virtual wire equal to the linear combination (this is the free unlimited fan-in of the [Bootle16 CS](https://tachyon.z.cash/ragu/protocol/core/arithmetization.html))
- `enforce_zero()` — constrains the linear combination to equal zero

---

### 5. The Routine trait

A `Routine` encapsulates a self-contained section of circuit logic with a **predict/execute** pattern:

- **`predict()`** — given the input gadget, try to determine the output without running the full circuit. Returns `Prediction::Known(output, aux)` if successful, `Prediction::Unknown(aux)` if not.
- **`execute()`** — run the full circuit with input, producing the output gadget.

This enables **memoization**: if two calls to the same routine have the same input wires, the SXY driver can reuse the predicted output and skip re-synthesis. The polynomial contribution of the routine is simply scaled by X^i · Y^j (see [Polynomial Management — Synthesis](https://tachyon.z.cash/ragu/implementation/polynomials.html)).

Routines are also the unit of **parallelization**: independent routines can execute concurrently during witness generation.

See [Routines](https://tachyon.z.cash/ragu/guide/routines.html).

---

### 6. The Emulator driver

`ragu_core` provides the `Emulator` driver with multiple modes:

- **`Emulator::execute()`** — `Wireless<Always, F>`: always has witness, no wire tracking. For running circuit code natively to compute outputs.
- **`Emulator::extractor()`** — `Wired<F>`: tracks wire assignments as `WiredValue<F>` (One or Assigned(F)). For extracting witness vectors.
- **`Emulator::counter()`** — `Wireless<Empty, F>`: no witness, no wires. For counting constraints.

The Emulator does **not** enforce constraints — it silently accepts any values. This is deliberate: it's for out-of-circuit execution, not verification.

> "The `Emulator` driver executes circuit code directly without enforcing constraints."
> — [Drivers](https://tachyon.z.cash/ragu/guide/drivers.html)

For implementation details see [Emulator](https://tachyon.z.cash/ragu/implementation/drivers/emulator.html).

---

### 7. The FromDriver trait

`FromDriver<'dr, 'new_dr, D>` enables converting a gadget's wires from one driver to another. This is used when the same circuit needs to be run under a different backend — for instance, extracting wire values computed by the RX driver for use in the SXY driver.

The trait provides:
- `convert_wire()` — map a single wire from the source to target driver
- `just()` — create a `DriverValue` in the target driver

---

### How it maps to the protocol

```
Multiplication constraints (a·b = c)  ← Driver::mul()
Linear constraints (Σ cᵢwᵢ = 0)      ← Driver::enforce_zero()
Virtual wires (free addition gates)    ← Driver::add() + LinearExpression
Witness generation (R(X) polynomial)   ← Always<T> + RX driver
Constraint synthesis (S(X,Y) poly)     ← Empty + SXY driver
Circuit-level values                   ← Gadget / GadgetKind
Polynomial synthesis memoization       ← Routine::predict/execute
Native execution without constraints   ← Emulator
```

### Reading order

1. **`src/maybe/mod.rs`** — `Maybe<T>`, `MaybeKind` traits
2. **`src/maybe/always.rs`** — `Always<T>` (witness present)
3. **`src/maybe/empty.rs`** — `Empty` (witness absent)
4. **`src/drivers/linexp.rs`** — `LinearExpression`, `DirectSum`
5. **`src/drivers.rs`** — `DriverTypes`, `Driver<'dr>`, `FromDriver`
6. **`src/drivers/emulator.rs`** — Emulator modes (Wired, Wireless)
7. **`src/drivers/phantom.rs`** — PhantomData driver (for type erasure)
8. **`src/gadgets.rs`** — `Gadget`, `GadgetKind` traits
9. **`src/gadgets/foreign.rs`** — impls for `()`, arrays, tuples, Box
10. **`src/routines.rs`** — `Routine`, `Prediction`
11. **`src/errors.rs`** — Error enum
