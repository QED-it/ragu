//! Proof and proof-carrying data structures.
//!
//! Defines the [`Proof`] structure containing trace polynomials, commitments,
//! and accumulated claims, along with [`Pcd`] which bundles a [`Proof`] with the
//! data that a [`Header`] succinctly encodes.

#![allow(dead_code)]

pub(crate) mod components;
pub(crate) use components::*;

use ff::Field;
use ragu_arithmetic::Cycle;
use ragu_circuits::{
    polynomials::{Rank, structured, unstructured},
    registry::CircuitIndex,
};
use ragu_primitives::vec::Len;

use alloc::vec;

use crate::components::claims::native::RxComponent;
use crate::components::endoscalar::NumStepsLen;
use crate::header::Header;
use crate::internal::nested::NUM_ENDOSCALING_POINTS;

/// Represents proof-carrying data, a recursive proof for the correctness of
/// some accompanying data.
pub struct Pcd<'source, C: Cycle, R: Rank, H: Header<C::CircuitField>> {
    /// The recursive proof for the accompanying data.
    pub proof: Proof<C, R>,

    /// Data needed to witness a [`Header`] within a [`Step`](super::Step).
    pub data: H::Data<'source>,
}

impl<C: Cycle, R: Rank, H: Header<C::CircuitField>> Clone for Pcd<'_, C, R, H> {
    fn clone(&self) -> Self {
        Pcd {
            proof: self.proof.clone(),
            data: self.data.clone(),
        }
    }
}

/// Represents a recursive proof for the correctness of some computation.
#[derive(Clone)]
pub struct Proof<C: Cycle, R: Rank> {
    pub(crate) application: Application<C, R>,
    pub(crate) preamble: Preamble<C, R>,
    pub(crate) s_prime: SPrime<C, R>,
    pub(crate) error_m: ErrorM<C, R>,
    pub(crate) error_n: ErrorN<C, R>,
    pub(crate) ab: AB<C, R>,
    pub(crate) query: Query<C, R>,
    pub(crate) f: F<C, R>,
    pub(crate) eval: Eval<C, R>,
    pub(crate) p: P<C, R>,
    pub(crate) challenges: Challenges<C>,
    pub(crate) circuits: InternalCircuits<C, R>,
}

impl<C: Cycle, R: Rank> Proof<C, R> {
    /// Augment a recursive proof with some data, described by a [`Header`].
    pub fn carry<H: Header<C::CircuitField>>(self, data: H::Data<'_>) -> Pcd<'_, C, R, H> {
        Pcd { proof: self, data }
    }

    /// Returns the native-field rx polynomial for the given [`RxComponent`].
    pub(crate) fn native_rx(
        &self,
        component: RxComponent,
    ) -> &structured::Polynomial<C::CircuitField, R> {
        use RxComponent::*;
        match component {
            AbA => &self.ab.native.a_poly,
            AbB => &self.ab.native.b_poly,
            Application => &self.application.rx,
            Hashes1 => &self.circuits.hashes_1_rx,
            Hashes2 => &self.circuits.hashes_2_rx,
            PartialCollapse => &self.circuits.partial_collapse_rx,
            FullCollapse => &self.circuits.full_collapse_rx,
            ComputeV => &self.circuits.compute_v_rx,
            Preamble => &self.preamble.native.rx,
            ErrorM => &self.error_m.native.rx,
            ErrorN => &self.error_n.native.rx,
            Query => &self.query.native.rx,
            Eval => &self.eval.native.rx,
        }
    }
}

impl<C: Cycle, R: Rank, const HEADER_SIZE: usize> crate::Application<'_, C, R, HEADER_SIZE> {
    pub(crate) fn trivial_pcd<'source>(&self) -> Pcd<'source, C, R, ()> {
        self.trivial_proof().carry(())
    }

    pub(crate) fn trivial_proof(&self) -> Proof<C, R> {
        let host_blind = C::CircuitField::ONE;
        let nested_blind = C::ScalarField::ONE;

        let zero_structured_host = structured::Polynomial::<C::CircuitField, R>::new();
        let zero_structured_nested = structured::Polynomial::<C::ScalarField, R>::new();
        let zero_unstructured = unstructured::Polynomial::<C::CircuitField, R>::new();

        let host_commitment =
            zero_structured_host.commit_to_affine(C::host_generators(self.params), host_blind);
        let nested_commitment = zero_structured_nested
            .commit_to_affine(C::nested_generators(self.params), nested_blind);

        Proof {
            application: Application {
                circuit_id: CircuitIndex::new(0),
                left_header: vec![C::CircuitField::ZERO; HEADER_SIZE],
                right_header: vec![C::CircuitField::ZERO; HEADER_SIZE],
                rx: zero_structured_host.clone(),
                blind: host_blind,
                commitment: host_commitment,
            },
            preamble: Preamble {
                native: NativePreamble {
                    rx: zero_structured_host.clone(),
                    blind: host_blind,
                    commitment: host_commitment,
                },
                bridge: BridgePreamble {
                    rx: zero_structured_nested.clone(),
                    blind: nested_blind,
                    commitment: nested_commitment,
                },
            },
            s_prime: SPrime {
                native: NativeSPrime {
                    registry_wx0_poly: zero_unstructured.clone(),
                    registry_wx0_blind: host_blind,
                    registry_wx0_commitment: host_commitment,
                    registry_wx1_poly: zero_unstructured.clone(),
                    registry_wx1_blind: host_blind,
                    registry_wx1_commitment: host_commitment,
                },
                bridge: BridgeSPrime {
                    rx: zero_structured_nested.clone(),
                    blind: nested_blind,
                    commitment: nested_commitment,
                },
            },
            error_m: ErrorM {
                native: NativeErrorM {
                    registry_wy_poly: zero_structured_host.clone(),
                    registry_wy_blind: host_blind,
                    registry_wy_commitment: host_commitment,
                    rx: zero_structured_host.clone(),
                    blind: host_blind,
                    commitment: host_commitment,
                },
                bridge: BridgeErrorM {
                    rx: zero_structured_nested.clone(),
                    blind: nested_blind,
                    commitment: nested_commitment,
                },
            },
            error_n: ErrorN {
                native: NativeErrorN {
                    rx: zero_structured_host.clone(),
                    blind: host_blind,
                    commitment: host_commitment,
                },
                bridge: BridgeErrorN {
                    rx: zero_structured_nested.clone(),
                    blind: nested_blind,
                    commitment: nested_commitment,
                },
            },
            ab: AB {
                native: NativeAB {
                    a_poly: zero_structured_host.clone(),
                    a_blind: host_blind,
                    a_commitment: host_commitment,
                    b_poly: zero_structured_host.clone(),
                    b_blind: host_blind,
                    b_commitment: host_commitment,
                    c: C::CircuitField::ZERO,
                },
                bridge: BridgeAB {
                    rx: zero_structured_nested.clone(),
                    blind: nested_blind,
                    commitment: nested_commitment,
                },
            },
            query: Query {
                native: NativeQuery {
                    registry_xy_poly: zero_unstructured.clone(),
                    registry_xy_blind: host_blind,
                    registry_xy_commitment: host_commitment,
                    rx: zero_structured_host.clone(),
                    blind: host_blind,
                    commitment: host_commitment,
                },
                bridge: BridgeQuery {
                    rx: zero_structured_nested.clone(),
                    blind: nested_blind,
                    commitment: nested_commitment,
                },
            },
            f: F {
                native: NativeF {
                    poly: zero_unstructured.clone(),
                    blind: host_blind,
                    commitment: host_commitment,
                },
                bridge: BridgeF {
                    rx: zero_structured_nested.clone(),
                    blind: nested_blind,
                    commitment: nested_commitment,
                },
            },
            eval: Eval {
                native: NativeEval {
                    rx: zero_structured_host.clone(),
                    blind: host_blind,
                    commitment: host_commitment,
                },
                bridge: BridgeEval {
                    rx: zero_structured_nested.clone(),
                    blind: nested_blind,
                    commitment: nested_commitment,
                },
            },
            p: P {
                native: NativeP {
                    poly: zero_unstructured.clone(),
                    blind: host_blind,
                    commitment: host_commitment,
                    v: C::CircuitField::ZERO,
                },
                nested: NestedP {
                    endoscalar_rx: zero_structured_nested.clone(),
                    points_rx: zero_structured_nested.clone(),
                    step_rxs: vec![
                        zero_structured_nested.clone();
                        NumStepsLen::<NUM_ENDOSCALING_POINTS>::len()
                    ],
                },
            },
            challenges: Challenges::trivial(),
            circuits: InternalCircuits {
                hashes_1_rx: zero_structured_host.clone(),
                hashes_1_blind: host_blind,
                hashes_1_commitment: host_commitment,
                hashes_2_rx: zero_structured_host.clone(),
                hashes_2_blind: host_blind,
                hashes_2_commitment: host_commitment,
                partial_collapse_rx: zero_structured_host.clone(),
                partial_collapse_blind: host_blind,
                partial_collapse_commitment: host_commitment,
                full_collapse_rx: zero_structured_host.clone(),
                full_collapse_blind: host_blind,
                full_collapse_commitment: host_commitment,
                compute_v_rx: zero_structured_host,
                compute_v_blind: host_blind,
                compute_v_commitment: host_commitment,
            },
        }
    }
}
