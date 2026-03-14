//! Commit to the polynomial query claims at various points (typically $x$,
//! $xz$, $w$).
//!
//! This creates the [`proof::Query`] component of the proof, which contains
//! claimed evaluations (corresponding to each polynomial query) usually at
//! points like $x$, $xz$, and $w$.
//!
//! This phase of the fuse operation is also used to commit to the $m(W, x, y)$
//! restriction.

use ff::Field;
use ragu_arithmetic::Cycle;
use ragu_circuits::{polynomials::Rank, staging::StageExt};
use ragu_core::{Result, drivers::Driver, maybe::Maybe};
use ragu_primitives::Element;
use rand::CryptoRng;

use crate::{
    Application, Proof,
    internal::{
        native::InternalCircuitValues, native::stages::query as native,
        nested::stages::query as nested,
    },
    proof,
};

impl<C: Cycle, R: Rank, const HEADER_SIZE: usize> Application<'_, C, R, HEADER_SIZE> {
    pub(super) fn compute_query<'dr, D, RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        w: &Element<'dr, D>,
        x: &Element<'dr, D>,
        y: &Element<'dr, D>,
        z: &Element<'dr, D>,
        error_m: &proof::ErrorM<C, R>,
        left: &Proof<C, R>,
        right: &Proof<C, R>,
    ) -> Result<(proof::Query<C, R>, native::Witness<C>)>
    where
        D: Driver<'dr, F = C::CircuitField>,
    {
        let (native_query, query_witness) =
            self.compute_native_query(rng, w, x, y, z, error_m, left, right)?;

        let bridge = self.compute_bridge_query(rng, &native_query)?;

        Ok((
            proof::Query {
                native: native_query,
                bridge,
            },
            query_witness,
        ))
    }

    fn compute_native_query<'dr, D, RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        w: &Element<'dr, D>,
        x: &Element<'dr, D>,
        y: &Element<'dr, D>,
        z: &Element<'dr, D>,
        error_m: &proof::ErrorM<C, R>,
        left: &Proof<C, R>,
        right: &Proof<C, R>,
    ) -> Result<(proof::NativeQuery<C, R>, native::Witness<C>)>
    where
        D: Driver<'dr, F = C::CircuitField>,
    {
        let w = *w.value().take();
        let x = *x.value().take();
        let y = *y.value().take();
        let xz = x * *z.value().take();

        let registry_xy_poly = self.native_registry.xy(x, y);
        let registry_xy_blind = C::CircuitField::random(&mut *rng);

        let query_witness = native::Witness {
            // TODO: these can all be evaluated at the same time; in fact,
            // that's what registry.xy is supposed to allow.
            fixed_registry: InternalCircuitValues::from_fn(|id| {
                registry_xy_poly.eval(id.circuit_index().omega_j())
            }),
            registry_wxy: registry_xy_poly.eval(w),
            left: native::ChildEvaluationsWitness::from_proof(
                left,
                w,
                x,
                xz,
                &registry_xy_poly,
                &error_m.native.registry_wy_poly,
            ),
            right: native::ChildEvaluationsWitness::from_proof(
                right,
                w,
                x,
                xz,
                &registry_xy_poly,
                &error_m.native.registry_wy_poly,
            ),
        };

        let rx = native::Stage::<C, R, HEADER_SIZE>::rx(&query_witness)?;
        let blind = C::CircuitField::random(&mut *rng);
        let host_gen = C::host_generators(self.params);
        let [registry_xy_commitment, commitment] = ragu_arithmetic::batch_to_affine([
            registry_xy_poly.commit(host_gen, registry_xy_blind),
            rx.commit(host_gen, blind),
        ]);

        Ok((
            proof::NativeQuery {
                registry_xy_poly,
                registry_xy_blind,
                registry_xy_commitment,
                rx,
                blind,
                commitment,
            },
            query_witness,
        ))
    }

    fn compute_bridge_query<RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        native: &proof::NativeQuery<C, R>,
    ) -> Result<proof::BridgeQuery<C, R>> {
        let nested_query_witness = nested::Witness {
            native_query: native.commitment,
            registry_xy: native.registry_xy_commitment,
        };
        let rx = nested::Stage::<C::HostCurve, R>::rx(&nested_query_witness)?;
        let blind = C::ScalarField::random(&mut *rng);
        let commitment = rx.commit_to_affine(C::nested_generators(self.params), blind);

        Ok(proof::BridgeQuery {
            rx,
            blind,
            commitment,
        })
    }
}
