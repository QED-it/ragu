//! Security audit verification tests for ragu_arithmetic.
//!
//! Each test targets one specific finding from the audit, demonstrating
//! the issue exists at the current code level.

use pasta_curves::{
    Fp as F,
    group::{Curve, prime::PrimeCurveAffine},
};
use ragu_arithmetic::{dot, mul};

///  MSM `mul()` silently truncates on length mismatch
///
/// A caller passing mismatched-length vectors gets a wrong
/// result with no error.
#[test]
fn msm_silent_truncation_on_length_mismatch() {
    let bases: Vec<pasta_curves::EqAffine> = (1u64..=5)
        .map(|i| (pasta_curves::EqAffine::generator() * F::from(i)).to_affine())
        .collect();

    // full 5element coefficient vector
    let full_coeffs: Vec<F> = (1u64..=5).map(F::from).collect();

    // 3element coefficient vector (caller mistake)
    let short_coeffs: Vec<F> = (1u64..=3).map(F::from).collect();

    // mul() with mismatched lengths: 3 coefficients, 5 bases
    let result_short = mul(short_coeffs.iter(), bases.iter());

    // compare against the correct full computation
    let result_full = mul(full_coeffs.iter(), bases.iter());

    // terms 4 and 5 were silently discarded
    assert_ne!(
        result_short, result_full,
        "MSM silently dropped terms, wrong result with no error"
    );

    let did_panic = std::panic::catch_unwind(|| {
        dot(short_coeffs.iter(), full_coeffs.iter());
    });
    assert!(
        did_panic.is_err(),
        "dot() correctly panics on length mismatch"
    );
}
