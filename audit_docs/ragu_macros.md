## Background for Understanding `ragu_macros`

### What the crate does

`ragu_macros` is a procedural macro crate that generates boilerplate for the gadget system. It has no runtime code — it only runs at compile time. Every `#[derive(Gadget)]` and `#[derive(Write)]` annotation in the project expands through this crate.

---

### 1. The Gadget derive macro

The `#[derive(Gadget)]` macro generates three impl blocks for a struct:

1. **`Clone`** — with special handling for `DriverValue` fields (uses `Maybe::clone()` instead of `T::clone()`)
2. **`Gadget<'dr, D>`** — sets the `Kind` associated type to the struct's "phantom form" (driver replaced with `PhantomData<F>`, lifetime set to `'static`)
3. **`unsafe impl GadgetKind<F>`** — the core transformation logic: `map_gadget` (converts wires between drivers) and `enforce_equal_gadget` (constrains two gadgets to be equal)

Field annotations control how each field is handled during driver conversion:

| Annotation | Field type | `map_gadget` behavior |
|---|---|---|
| `#[ragu(wire)]` | `D::Wire` | `FromDriver::convert_wire()` |
| `#[ragu(value)]` | `DriverValue<D, T>` | Clone witness via `Maybe::view().take()` |
| `#[ragu(gadget)]` | nested `Gadget` | Recursive `Gadget::map()` |
| `#[ragu(phantom)]` | `PhantomData` | Replaced with `PhantomData` |

The gadget system is central to circuit construction. See [Writing Circuits](https://tachyon.z.cash/ragu/guide/writing_circuits.html), [Gadgets](https://tachyon.z.cash/ragu/guide/gadgets/index.html), and [Simple Gadgets](https://tachyon.z.cash/ragu/guide/gadgets/simple.html).

---

### 2. The GadgetKind trait and Kind! macro

`GadgetKind<F>` is the driver-agnostic type-level identity of a gadget. It uses a GAT (`Rebind<'dr, D>`) to recover the concrete gadget type for any driver. The `Kind!` macro provides syntactic sugar:

```
Kind![F; MyGadget<'_, _>]
// expands to:
<MyGadget<'static, PhantomData<F>> as Gadget<'static, PhantomData<F>>>::Kind
```

The macro replaces `'_` with `'static` and `_` with `PhantomData<F>`, producing the phantom form used as the `Kind` associated type.

See [The GadgetKind Trait](https://tachyon.z.cash/ragu/guide/gadgets/gadgetkind.html) and [The Kind! Macro](https://tachyon.z.cash/ragu/guide/gadgets/kind.html).

---

### 3. The Write derive macro

`#[derive(Write)]` generates serialization of gadget fields into a `Buffer` of `Element` values. This is used for encoding gadgets into polynomials for commitment.

- Default fields: serialized via `GadgetExt::write()`
- `#[ragu(skip)]` or `#[ragu(phantom)]`: omitted from serialization

This connects to the [Polynomial Management](https://tachyon.z.cash/ragu/implementation/polynomials.html) layer where gadget outputs are serialized into the public input polynomial k(Y).

---

### 4. repr256! macro

Converts a large integer literal (up to 2^256 - 1) into a `[u64; 4]` little-endian array at compile time. Used by the `fp!` and `fq!` macros in `ragu_pasta` to embed field element constants (Poseidon round constants, MDS matrix entries) directly in source.

---

### 5. impl_maybe_cast_tuple! macro

Generates `MaybeCast` trait implementations for tuples of sizes 2 through N. This enables distributing a `Maybe<(A, B, C)>` into `(Maybe<A>, Maybe<B>, Maybe<C>)` — essential for the [Maybe monad](https://tachyon.z.cash/ragu/guide/drivers.html) that separates witness generation from constraint synthesis.

---

### How it maps to the protocol

```
Gadget fungibility / wire mapping  ← #[derive(Gadget)]
Driver-agnostic gadget identity    ← Kind! macro
Gadget serialization to polynomials ← #[derive(Write)]
Field constant embedding           ← repr256!
Maybe<T> tuple destructuring       ← impl_maybe_cast_tuple!
```

### Reading order

1. **`src/lib.rs`** — macro entry points and exports
2. **`src/helpers.rs`** — GenericDriver extraction, attribute parsing
3. **`src/derive/gadget.rs`** — the Gadget derive (most complex)
4. **`src/derive/gadgetwrite.rs`** — the Write derive
5. **`src/proc/kind.rs`** — Kind! macro
6. **`src/proc/repr.rs`** — repr256! macro
7. **`src/proc/maybe_cast.rs`** — tuple MaybeCast generation
8. **`src/substitution.rs`** — AST rewriting for phantom form
9. **`src/path_resolution.rs`** — crate path resolution for hygiene
