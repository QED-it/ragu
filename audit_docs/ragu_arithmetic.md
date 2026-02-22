## Background for Understanding `ragu_arithmetic`

### What the crate does

`ragu_arithmetic` provides the mathematical primitives that the entire proof system is built on. It's the lowest non-macro crate in the dependency tree. Every proof, commitment, and verification in the upper crates ultimately reduces to operations defined here.

---

### 1. Finite fields

All arithmetic operates over **finite fields** F, not integers. The Pasta cycle gives two prime fields Fp and Fq, where each curve's scalar field equals the other's base field. This relationship is encoded by the `Cycle` trait in `lib.rs`.

The `ff::Field` and `ff::PrimeField` traits provide the interface. Every value you see in this crate — polynomial coefficients, evaluations, scalars — lives in one of these fields.

> See [Notation](https://tachyon.z.cash/ragu/protocol/prelim/notation.html) — "Group elements in uppercase, scalars and field elements in lowercase."

---

### 2. Polynomials

Polynomials are represented as coefficient vectors: `[a₀, a₁, a₂]` means a₀ + a₁X + a₂X². The book establishes this convention:

> "Given a univariate polynomial p in F[X] of maximal degree n-1 there exists a unique coefficient vector **p** = (p₀, p₁, ..., p_{n-1}), ordered from lowest to highest degree."
> — [Notation](https://tachyon.z.cash/ragu/protocol/prelim/notation.html)

Key operations in `util.rs`:

- **`eval()`** — Horner's method. Equivalent to the dot product <**p**, **z^n**> from the book's notation.
- **`factor()`** — Synthetic division: given that p(r) = 0, extracts quotient q(X) where p(X) = (X - r)·q(X). This is used directly in the PCS batched evaluation scheme where the prover computes quotient polynomials q_i(X) = (p_i(X) - y_i) / (X - x_i) for each opening claim (see [PCS Batched Evaluation](https://tachyon.z.cash/ragu/protocol/core/accumulation/pcs.html), step 2).
- **`dot()`** — Inner product <**a**, **b**> = sum(a_i · b_i). The book's central operation — it appears in the Bulletproofs IPA relation, in the revdot product, and in the consolidated constraint equation.
- **`geosum()`** — Computes 1 + r + r² + ... + r^(m-1) via repeated doubling. Used throughout the protocol for power-vector operations (**z^n** notation in the book).

---

### 3. The revdot product

The **revdot product** revdot(**a**, **b**) = dot(**a**, reverse(**b**)) is not a separate function in `ragu_arithmetic` — it composes `dot()` with vector reversal. But it is the single most important operation in the protocol. The entire [NARK](https://tachyon.z.cash/ragu/protocol/core/nark.html) reduces to one consolidated constraint:

> revdot(**r**, **r** . **z^4n** - **t** + **s**) = dot(**k**, **y^4n**)

The connection to polynomial multiplication is the key insight: when you multiply two polynomials p(X)·q(X), the coefficient of X^(n-1) in the product equals revdot(**p**, **q**). The [NARK](https://tachyon.z.cash/ragu/protocol/core/nark.html) exploits this by decomposing the product polynomial into two halves c₁(X) and c₂(X) (each degree < 4n), then extracting the revdot value as c₁(0). This decomposition requires polynomial reversal — the `factor()` and coefficient-reversal patterns from `util.rs`.

The revdot product has its own [accumulation scheme](https://tachyon.z.cash/ragu/protocol/core/accumulation/revdot.html) with a two-layer reduction that brings constraints from (MN)² down to NM² + N² - N + 3.

---

### 4. FFT and evaluation domains

The `Domain<F>` type represents a **multiplicative subgroup** {1, ω, ω², ..., ω^(n-1)} where ω is a primitive n-th root of unity and n = 2^k.

**Why it matters**: The NARK prover must compute the product polynomial r̂(X)·r(zX), which the book describes as the most expensive prover step:

> "We need at least 3 FFTs over a domain of size 8n-2. This is the most expensive step for the prover."
> — [NARK](https://tachyon.z.cash/ragu/protocol/core/nark.html)

The FFT converts between coefficient and evaluation representations in O(d log d), enabling efficient polynomial multiplication. The book also notes FFT use in the [structured vectors](https://tachyon.z.cash/ragu/protocol/prelim/structured_vectors.html) reduction.

`fft.rs` implements the **radix-2 Cooley-Tukey DIT** butterfly. `Domain` wraps it with `fft()`, `ifft()`, and **`ell()`** — barycentric Lagrange interpolation for evaluating a polynomial (given as domain evaluations) at an arbitrary point outside the domain.

**`ell()` and the registry**: The [Registry Polynomial](https://tachyon.z.cash/ragu/protocol/extensions/registry.html) m(W, X, Y) interpolates all circuit wiring polynomials so that m(ω^i, X, Y) = s_i(X, Y). Evaluating the registry at an arbitrary challenge point w requires Lagrange coefficients:

> ℓ_i(w) = ∏_{j≠i} (w - ω^j) / (ω^i - ω^j)

This is exactly what `Domain::ell()` computes — the barycentric weights over the domain of roots of unity.

Pasta fields have 2-adicity S = 32, so the largest valid domain is 2^32 elements.

---

### 5. `bitreverse` and the rolling registry

The `bitreverse(v, num_bits)` function in `fft.rs` reverses the bit pattern of an index. Beyond its standard FFT role, it serves a crucial purpose in the [registry construction](https://tachyon.z.cash/ragu/protocol/extensions/registry.html):

Circuits are assigned to domain points incrementally at compile time, before the final registry size is known. The trick: assign circuit j to ω_S^bitreverse(j, S) in the maximal domain. When the registry finalizes at size 2^k, each point maps to the smaller domain via i' = i >> (S-k). This works because ω_k = ω_S^(2^(S-k)).

> "Circuit synthesis can compute ω_S^i at compile-time without knowing how many other circuits will be registered in the registry."
> — [Registry Polynomial](https://tachyon.z.cash/ragu/protocol/extensions/registry.html)

---

### 6. Multi-scalar multiplication (MSM)

`mul()` computes: result = s₁·P₁ + s₂·P₂ + ... + sₙ·Pₙ

This is the **dominant cost** in the protocol. It appears in two critical places described in the book:

1. **Polynomial commitment** — Pedersen vector commitment: F = <**f**, **G**>, committing a polynomial's coefficient vector against generators. (See [Bulletproofs IPA](https://tachyon.z.cash/ragu/protocol/prelim/bulletproofs.html) — "we commit the polynomial using Pedersen commitment over its coefficient")

2. **IPA verification** — Computing the folded generators G₀ = <**s**, **G**> and H₀ = <**s⁻¹**, **H**> is "the dominant verifier cost and the culprit of the linear-time verifier" ([Bulletproofs IPA](https://tachyon.z.cash/ragu/protocol/prelim/bulletproofs.html)). This cost motivates the [split-accumulation](https://tachyon.z.cash/ragu/protocol/core/accumulation/index.html) scheme that defers it.

3. **Accumulation verifier** — Folding commitments via random linear combinations requires MSM: P̄ = F̄ + Σ βⁱ·C̄ᵢ. The [analysis](https://tachyon.z.cash/ragu/protocol/analysis.html) notes "Multi-scalar multiplication size: 8192 group elements... dominates the verifier's computational cost."

The implementation uses **Pippenger's bucket method**: groups scalar bits into windows of size c, accumulates points into buckets per window, then reduces. This brings complexity from O(n·256) down to roughly O(n + 2^c · 256/c).

---

### 7. Coeff — tagged field elements

`Coeff<F>` wraps field elements with special-case tags: Zero, One, Two, NegativeOne, Arbitrary, NegativeArbitrary. This avoids unnecessary field multiplications during circuit synthesis.

The book explains why this matters in [Drivers](https://tachyon.z.cash/ragu/guide/drivers.html):

> "In some frameworks, circuit synthesis alone accounts for 25-30% of the proof generation time."

And in [Polynomial Management](https://tachyon.z.cash/ragu/implementation/polynomials.html), synthesis is described as procedural — repeated sequences of `enforce_zero` and `mul` operations produce the wiring polynomial S(X,Y). The `Coeff` tags let the SXY driver skip multiplications by 0 or 1, which dominate the sparse wiring matrices **u**, **v**, **w** from the [arithmetization](https://tachyon.z.cash/ragu/protocol/core/arithmetization.html).

---

### 8. Endoscalars and `uendo.rs`

An **endoscalar** is a small binary string (128 or 136 bits) that serves as a cross-circuit scalar — it can be efficiently mapped to both Fp and Fq. The [endoscalar](https://tachyon.z.cash/ragu/protocol/extensions/endoscalar.html) interface requires three operations:

- **extract**(s ∈ F) → endo(s): deterministically extract λ bits from a field element
- **lift**(endo(s)) → s ∈ F: lift back to a (possibly different) target field
- **endoscaling**: endo(s)·G ∈ G, scalar multiplication using the curve's efficient endomorphism

This solves a fundamental problem: when a verifier challenge α ∈ Fp needs to multiply a group element G ∈ G₁, that group operation is expensive in an Fp-circuit. Instead, extract an endoscalar in the Fp-circuit, pass it to the Fq-circuit, and do the multiplication natively there. See [PCS — Transcript Bridging](https://tachyon.z.cash/ragu/protocol/core/accumulation/pcs.html).

The `uendo.rs` file implements the 136-bit `Uendo` type for this purpose, though it is currently dead code (`lib.rs` re-exports `u128` as `Endoscalar` instead).

---

### 9. The `Cycle`, `PoseidonPermutation`, and `FixedGenerators` traits

The **`Cycle`** trait in `lib.rs` encodes the Pasta curve relationship: Pallas and Vesta form a 2-cycle where each curve's base field is the other's scalar field. This enables the [CycleFold-inspired](https://tachyon.z.cash/ragu/protocol/core/accumulation/pcs.html) recursion design where accumulation work is split between two merge circuits — evaluations are folded in their field-native circuit while commitments are folded in the other.

**`PoseidonPermutation`** defines the algebraic hash used for Fiat-Shamir throughout. Ragu models Poseidon as a [random oracle](https://tachyon.z.cash/ragu/protocol/prelim/assumptions.html), and uses it [prolifically](https://tachyon.z.cash/ragu/protocol/index.html) for non-interactive challenge derivation. The trait specifies T (width), RATE, sbox degree, and round counts but does not enforce constraints at compile time — these are safe trait contracts.

**`FixedGenerators`** provides the pre-computed generator points **G** ∈ G^n used in Pedersen vector commitments. These are the points that MSM multiplies against.

---

### How it maps to the protocol

```
NARK consolidated constraint    ← dot() (revdot = dot + reversal)
Polynomial product decomposition ← fft/ifft, eval()
PCS batched opening (quotients)  ← factor()
Pedersen commitments / IPA       ← mul() (MSM)
Registry interpolation           ← Domain::ell() (barycentric)
Rolling circuit registration     ← bitreverse()
Wiring polynomial S(X,Y)        ← Coeff (optimized synthesis)
Power vectors z^n                ← geosum()
Cross-circuit scalars            ← uendo.rs (Endoscalar type)
Curve cycle recursion            ← Cycle trait
Fiat-Shamir transcript           ← PoseidonPermutation trait
Commitment generators            ← FixedGenerators trait
```

### Reading order

1. **`lib.rs`** — trait definitions (Cycle, PoseidonPermutation, FixedGenerators, CurveAffine re-export)
2. **`coeff.rs`** — Coeff enum and optimized arithmetic
3. **`domain.rs`** — evaluation domains, FFT integration, barycentric interpolation (ell)
4. **`fft.rs`** — raw FFT butterfly + bitreverse
5. **`util.rs`** — polynomial operations (eval, factor, dot, geosum) and MSM (mul)
6. **`uendo.rs`** — 136-bit endoscalar type (currently dead code, `u128` used instead)
