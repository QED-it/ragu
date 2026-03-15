use ff::Field;
use ragu_arithmetic::Cycle;
use ragu_circuits::{CircuitExt, polynomials::Rank};
use ragu_core::Result;
use rand::CryptoRng;

use crate::{
    Application,
    internal::{native, native::total_circuit_counts},
    proof,
};

impl<C: Cycle, R: Rank, const HEADER_SIZE: usize> Application<'_, C, R, HEADER_SIZE> {
    pub(super) fn compute_internal_circuits<RNG: CryptoRng>(
        &self,
        rng: &mut RNG,
        preamble: &proof::Preamble<C, R>,
        s_prime: &proof::SPrime<C, R>,
        error_n: &proof::ErrorN<C, R>,
        error_m: &proof::ErrorM<C, R>,
        ab: &proof::AB<C, R>,
        query: &proof::Query<C, R>,
        f: &proof::F<C, R>,
        eval: &proof::Eval<C, R>,
        p: &proof::P<C, R>,
        preamble_witness: &native::stages::preamble::Witness<'_, C, R, HEADER_SIZE>,
        error_n_witness: &native::stages::error_n::Witness<C, native::RevdotParameters>,
        error_m_witness: &native::stages::error_m::Witness<C, native::RevdotParameters>,
        query_witness: &native::stages::query::Witness<C>,
        eval_witness: &native::stages::eval::Witness<C::CircuitField>,
        challenges: &proof::Challenges<C>,
    ) -> Result<proof::InternalCircuits<C, R>> {
        let unified = native::unified::Instance {
            bridge_preamble_commitment: preamble.bridge.commitment,
            w: challenges.w,
            bridge_s_prime_commitment: s_prime.bridge.commitment,
            y: challenges.y,
            z: challenges.z,
            bridge_error_m_commitment: error_m.bridge.commitment,
            mu: challenges.mu,
            nu: challenges.nu,
            bridge_error_n_commitment: error_n.bridge.commitment,
            mu_prime: challenges.mu_prime,
            nu_prime: challenges.nu_prime,
            c: ab.native.c,
            bridge_ab_commitment: ab.bridge.commitment,
            x: challenges.x,
            bridge_query_commitment: query.bridge.commitment,
            alpha: challenges.alpha,
            bridge_f_commitment: f.bridge.commitment,
            u: challenges.u,
            bridge_eval_commitment: eval.bridge.commitment,
            pre_beta: challenges.pre_beta,
            v: p.native.v,
            coverage: Default::default(),
        };

        let (hashes_1_trace, unified) = native::circuits::hashes_1::Circuit::<
            C,
            R,
            HEADER_SIZE,
            native::RevdotParameters,
        >::new(
            self.params,
            total_circuit_counts(self.num_application_steps).1,
        )
        .rx(native::circuits::hashes_1::Witness {
            unified,
            preamble_witness,
            error_n_witness,
        })?;
        let hashes_1_rx = self.native_registry.assemble(
            &hashes_1_trace,
            native::InternalCircuitIndex::Hashes1Circuit.circuit_index(),
        )?;
        let hashes_1_rx_blind = C::CircuitField::random(&mut *rng);

        let (hashes_2_trace, unified) = native::circuits::hashes_2::Circuit::<
            C,
            R,
            HEADER_SIZE,
            native::RevdotParameters,
        >::new(self.params)
        .rx(native::circuits::hashes_2::Witness {
            unified,
            error_n_witness,
        })?;
        let hashes_2_rx = self.native_registry.assemble(
            &hashes_2_trace,
            native::InternalCircuitIndex::Hashes2Circuit.circuit_index(),
        )?;
        let hashes_2_rx_blind = C::CircuitField::random(&mut *rng);

        let (partial_collapse_trace, unified) = native::circuits::partial_collapse::Circuit::<
            C,
            R,
            HEADER_SIZE,
            native::RevdotParameters,
        >::new()
        .rx(native::circuits::partial_collapse::Witness {
            preamble_witness,
            unified,
            error_n_witness,
            error_m_witness,
        })?;
        let partial_collapse_rx = self.native_registry.assemble(
            &partial_collapse_trace,
            native::InternalCircuitIndex::PartialCollapseCircuit.circuit_index(),
        )?;
        let partial_collapse_rx_blind = C::CircuitField::random(&mut *rng);

        let (full_collapse_trace, unified) = native::circuits::full_collapse::Circuit::<
            C,
            R,
            HEADER_SIZE,
            native::RevdotParameters,
        >::new()
        .rx(native::circuits::full_collapse::Witness {
            unified,
            preamble_witness,
            error_n_witness,
        })?;
        let full_collapse_rx = self.native_registry.assemble(
            &full_collapse_trace,
            native::InternalCircuitIndex::FullCollapseCircuit.circuit_index(),
        )?;
        let full_collapse_rx_blind = C::CircuitField::random(&mut *rng);

        let (compute_v_trace, unified) =
            native::circuits::compute_v::Circuit::<C, R, HEADER_SIZE>::new().rx(
                native::circuits::compute_v::Witness {
                    unified,
                    preamble_witness,
                    query_witness,
                    eval_witness,
                },
            )?;
        let compute_v_rx = self.native_registry.assemble(
            &compute_v_trace,
            native::InternalCircuitIndex::ComputeVCircuit.circuit_index(),
        )?;
        let compute_v_rx_blind = C::CircuitField::random(&mut *rng);

        // Cross-circuit coverage validation (prover-time development assertion,
        // not a verifier check): all internal recursion circuits together must
        // cover every slot exactly once. Overlap is caught eagerly by finish();
        // missing slots are caught here.
        unified.assert_complete();

        let host_gen = C::host_generators(self.params);
        let [
            hashes_1_rx_commitment,
            hashes_2_rx_commitment,
            partial_collapse_rx_commitment,
            full_collapse_rx_commitment,
            compute_v_rx_commitment,
        ] = ragu_arithmetic::batch_to_affine([
            hashes_1_rx.commit(host_gen, hashes_1_rx_blind),
            hashes_2_rx.commit(host_gen, hashes_2_rx_blind),
            partial_collapse_rx.commit(host_gen, partial_collapse_rx_blind),
            full_collapse_rx.commit(host_gen, full_collapse_rx_blind),
            compute_v_rx.commit(host_gen, compute_v_rx_blind),
        ]);

        Ok(proof::InternalCircuits {
            hashes_1_rx,
            hashes_1_blind: hashes_1_rx_blind,
            hashes_1_commitment: hashes_1_rx_commitment,
            hashes_2_rx,
            hashes_2_blind: hashes_2_rx_blind,
            hashes_2_commitment: hashes_2_rx_commitment,
            partial_collapse_rx,
            partial_collapse_blind: partial_collapse_rx_blind,
            partial_collapse_commitment: partial_collapse_rx_commitment,
            full_collapse_rx,
            full_collapse_blind: full_collapse_rx_blind,
            full_collapse_commitment: full_collapse_rx_commitment,
            compute_v_rx,
            compute_v_blind: compute_v_rx_blind,
            compute_v_commitment: compute_v_rx_commitment,
        })
    }
}
