//! Security audit verification tests for ragu_arithmetic.
//!
//! Each test targets one specific finding from the audit, demonstrating
//! the issue exists at the current code level.

use ff::PrimeField;
use pasta_curves::{
    Fp as F,
    group::{Curve, prime::PrimeCurveAffine},
};
use ragu_arithmetic::{Domain, bitreverse, dot, factor, factor_iter, mul};

///  MSM `mul()` silently truncates on length mismatch
///
/// A caller passing mismatched-length vectors gets a wrong
/// result with no error.
#[test]
fn msm_silent_truncation_on_length_mismatch() {
    let bases: Vec<pasta_curves::EqAffine> = (1u64..=5)
        .map(|i| (pasta_curves::EqAffine::generator() * F::from(i)).to_affine())
        .collect();

    // full 5-element coefficient vector
    let full_coeffs: Vec<F> = (1u64..=5).map(F::from).collect();

    // 3-element coefficient vector (caller mistake)
    let short_coeffs: Vec<F> = (1u64..=3).map(F::from).collect();

    // mul() with mismatched lengths: 3 coefficients, 5 bases
    let result_short = mul(short_coeffs.iter(), bases.iter());

    // compare against the correct full computation
    let result_full = mul(full_coeffs.iter(), bases.iter());

    // terms 4 and 5 were silently discarded.
    assert_ne!(
        result_short, result_full,
        "MSM silently dropped terms, wrong result with no error"
    );

    // Contrast with dot(), which correctly panics on length mismatch:
    let did_panic = std::panic::catch_unwind(|| {
        dot(short_coeffs.iter(), full_coeffs.iter());
    });
    assert!(
        did_panic.is_err(),
        "dot() correctly panics on length mismatch"
    );
}

/// **Finding 3 — FFT `fft()` casts `input.len()` to `u32`, wrapping to 0
/// for domain size 2^32**
///
/// Pasta fields have 2-adicity S=32, so `Domain::new(32)` is valid and
/// produces n = 2^32. Inside `fft()`, `let n = input.len() as u32`
/// truncates 2^32 to 0, causing the FFT to silently return the input
/// unmodified. We cannot allocate 2^32 elements in a test, but we can
/// demonstrate the truncation that causes the bug.
#[test]
fn audit_fft_u32_truncation_at_domain_size_2_32() {
    // Pasta Fp has 2-adicity S = 32, so the maximum domain is 2^32.
    assert_eq!(F::S, 32, "Pasta Fp 2-adicity should be 32");

    // Domain::new(32) is valid — it constructs a domain of size 2^32.
    let domain = Domain::<F>::new(32);
    assert_eq!(domain.n(), 1usize << 32);
    assert_eq!(domain.log2_n(), 32);

    // The bug: inside fft(), `let n = input.len() as u32` wraps 2^32 → 0
    let n_as_usize: usize = 1 << 32;
    let n_as_u32: u32 = n_as_usize as u32;
    assert_eq!(
        n_as_u32, 0,
        "casting 2^32 to u32 wraps to 0 — this is the root of the FFT bug"
    );

    // Consequence: `for i in 0..n` iterates zero times → bit-reversal is
    // skipped, all butterfly stages read n=0, the FFT returns its input
    // unchanged. The result is mathematically wrong with no error raised.

    // Also, `bitreverse(i, 32)` is called with log2_n=32, processing all
    // 32 bits, which is fine — but the enclosing loop `for i in 0..0`
    // never executes it.

    // Additional verification: bitreverse works correctly for log2_n < 32
    assert_eq!(bitreverse(0b1010, 4), 0b0101);
    assert_eq!(bitreverse(1, 31), 1 << 30);
}

/// factor() and factor_iter() silently returns empty result
/// for degree-0 polynomials instead of panicking as documented
#[test]
fn factor_degree_zero_no_panic() {
    let x = F::from(7);

    // correctly panics.
    let did_panic = std::panic::catch_unwind(|| {
        let empty: Vec<F> = vec![];
        factor(empty.into_iter(), x)
    });
    assert!(did_panic.is_err(), "empty polynomial should panic");

    // Degree-0 polynomial, no panic
    let degree_0 = vec![F::from(42)];
    let result = factor(degree_0.into_iter(), x);
    assert!(
        result.is_empty(),
        "factor of degree-0 poly returned {:?} instead of panicking",
        result
    );

    // Same for factor_iter
    let degree_0 = vec![F::from(42)];
    let result_iter: Vec<F> = factor_iter(degree_0.into_iter(), x).collect();
    assert!(
        result_iter.is_empty(),
        "factor_iter of degree-0 poly returned {:?} instead of panicking",
        result_iter
    );
}
