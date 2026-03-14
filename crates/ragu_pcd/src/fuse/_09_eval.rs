//! Commit to the evaluations of every queried polynomial at $u$.
//!
//! This creates the [`proof::Eval`] component of the proof, which contains
//! evaluations of every committed or accumulated polynomial (thus far) at the
//! point $u$, except $f(u)$ which is _derived_ from said evaluations.

use ff::Field;
use ragu_arithmetic::Cycle;
use ragu_circuits::{polynomials::Rank, staging::StageExt};
use ragu_core::{Result, drivers::Driver, maybe::Maybe};
use ragu_primitives::Element;
use rand::CryptoRng;

use crate::{
    Application, Proof,
    circuits::{native::stages::eval as native, nested::stages::eval as nested},
    proof,
};

impl<C: Cycle, R: Rank, const HEADER_SIZE: usize> Application<'_, C, R, HEADER_SIZE> {
    pub(super) fn compute_eval<'dr, D, RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        u: &Element<'dr, D>,
        left: &Proof<C, R>,
        right: &Proof<C, R>,
        s_prime: &proof::SPrime<C, R>,
        error_m: &proof::ErrorM<C, R>,
        ab: &proof::AB<C, R>,
        query: &proof::Query<C, R>,
    ) -> Result<(proof::Eval<C, R>, native::Witness<C::CircuitField>)>
    where
        D: Driver<'dr, F = C::CircuitField>,
    {
        let (native_eval, eval_witness) =
            self.compute_native_eval(rng, u, left, right, s_prime, error_m, ab, query)?;

        let bridge = self.compute_bridge_eval(rng, &native_eval)?;

        Ok((
            proof::Eval {
                native: native_eval,
                bridge,
            },
            eval_witness,
        ))
    }

    fn compute_native_eval<'dr, D, RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        u: &Element<'dr, D>,
        left: &Proof<C, R>,
        right: &Proof<C, R>,
        s_prime: &proof::SPrime<C, R>,
        error_m: &proof::ErrorM<C, R>,
        ab: &proof::AB<C, R>,
        query: &proof::Query<C, R>,
    ) -> Result<(proof::NativeEval<C, R>, native::Witness<C::CircuitField>)>
    where
        D: Driver<'dr, F = C::CircuitField>,
    {
        let u = *u.value().take();

        let eval_witness = native::Witness {
            left: native::ChildEvaluationsWitness::from_proof(left, u),
            right: native::ChildEvaluationsWitness::from_proof(right, u),
            current: native::CurrentStepWitness {
                // TODO: the registry evaluations here could _theoretically_ be more
                // efficient if they're computed simultaneously with assistance
                // from the registry itself, rather than individually evaluated for
                // each of these restrictions.
                registry_wx0: s_prime.native.registry_wx0_poly.eval(u),
                registry_wx1: s_prime.native.registry_wx1_poly.eval(u),
                registry_wy: error_m.native.registry_wy_poly.eval(u),
                a_poly: ab.native.a_poly.eval(u),
                b_poly: ab.native.b_poly.eval(u),
                registry_xy: query.native.registry_xy_poly.eval(u),
            },
        };
        let rx = native::Stage::<C, R, HEADER_SIZE>::rx(&eval_witness)?;
        let blind = C::CircuitField::random(&mut *rng);
        let commitment = rx.commit_to_affine(C::host_generators(self.params), blind);

        Ok((
            proof::NativeEval {
                rx,
                blind,
                commitment,
            },
            eval_witness,
        ))
    }

    fn compute_bridge_eval<RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        native: &proof::NativeEval<C, R>,
    ) -> Result<proof::BridgeEval<C, R>> {
        let nested_eval_witness = nested::Witness {
            native_eval: native.commitment,
        };
        let rx = nested::Stage::<C::HostCurve, R>::rx(&nested_eval_witness)?;
        let blind = C::ScalarField::random(&mut *rng);
        let commitment = rx.commit_to_affine(C::nested_generators(self.params), blind);

        Ok(proof::BridgeEval {
            rx,
            blind,
            commitment,
        })
    }
}
