//! Commit to the preamble.
//!
//! This creates the [`proof::Preamble`] component of the proof, which commits
//! to the instance and trace polynomials used in the fuse step.

use ff::Field;
use ragu_arithmetic::Cycle;
use ragu_circuits::{polynomials::Rank, staging::StageExt};
use ragu_core::Result;
use rand::CryptoRng;

use crate::{
    Application, Proof,
    internal::{native::stages::preamble as native, nested::stages::preamble as nested},
    proof,
};

impl<C: Cycle, R: Rank, const HEADER_SIZE: usize> Application<'_, C, R, HEADER_SIZE> {
    pub(super) fn compute_preamble<'a, RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        left: &'a Proof<C, R>,
        right: &'a Proof<C, R>,
        application: &proof::Application<C, R>,
    ) -> Result<(
        proof::Preamble<C, R>,
        native::Witness<'a, C, R, HEADER_SIZE>,
    )> {
        let (native, preamble_witness) =
            self.compute_native_preamble(rng, left, right, application)?;

        let bridge = self.compute_bridge_preamble(rng, &native, left, right)?;

        Ok((proof::Preamble { native, bridge }, preamble_witness))
    }

    fn compute_native_preamble<'a, RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        left: &'a Proof<C, R>,
        right: &'a Proof<C, R>,
        application: &proof::Application<C, R>,
    ) -> Result<(
        proof::NativePreamble<C, R>,
        native::Witness<'a, C, R, HEADER_SIZE>,
    )> {
        let preamble_witness = native::Witness::new(
            left,
            right,
            &application.left_header,
            &application.right_header,
        )?;

        let rx = native::Stage::<C, R, HEADER_SIZE>::rx(&preamble_witness)?;
        let blind = C::CircuitField::random(&mut *rng);
        let commitment = rx.commit_to_affine(C::host_generators(self.params), blind);

        Ok((
            proof::NativePreamble {
                rx,
                blind,
                commitment,
            },
            preamble_witness,
        ))
    }

    fn compute_bridge_preamble<RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        native: &proof::NativePreamble<C, R>,
        left: &Proof<C, R>,
        right: &Proof<C, R>,
    ) -> Result<proof::BridgePreamble<C, R>> {
        let nested_preamble_witness = nested::Witness {
            native_preamble: native.commitment,
            left: nested::ChildWitness::from_proof(left),
            right: nested::ChildWitness::from_proof(right),
        };
        let rx = nested::Stage::<C::HostCurve, R>::rx(&nested_preamble_witness)?;
        let blind = C::ScalarField::random(&mut *rng);
        let commitment = rx.commit_to_affine(C::nested_generators(self.params), blind);

        Ok(proof::BridgePreamble {
            rx,
            blind,
            commitment,
        })
    }
}
