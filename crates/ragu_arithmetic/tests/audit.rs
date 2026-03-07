use pasta_curves::Fp as F;
use ragu_arithmetic::{factor, factor_iter};

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
