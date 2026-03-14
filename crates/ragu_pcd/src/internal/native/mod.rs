//! Native curve circuits for recursive verification.

use ragu_arithmetic::Cycle;
use ragu_circuits::{
    polynomials::Rank,
    registry::{CircuitIndex, RegistryBuilder},
};
use ragu_core::Result;

use super::NativeParameters;
use crate::step;

pub mod stages;

pub(crate) mod compute_v;
pub(crate) mod full_collapse;
pub(crate) mod hashes_1;
pub(crate) mod hashes_2;
pub(crate) mod partial_collapse;
pub(crate) mod unified;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InternalCircuitIndex {
    // Native stages
    PreambleStage,
    ErrorMStage,
    ErrorNStage,
    QueryStage,
    EvalStage,
    // Final stage masks
    ErrorMFinalStaged,
    ErrorNFinalStaged,
    EvalFinalStaged,
    // Native circuits
    Hashes1Circuit,
    Hashes2Circuit,
    PartialCollapseCircuit,
    FullCollapseCircuit,
    ComputeVCircuit,
}

/// The number of internal circuits registered by [`register_all`],
/// equal to the number of variants in [`InternalCircuitIndex`].
pub(crate) const NUM_INTERNAL_CIRCUITS: usize = 13;

/// Compute the total circuit count and log2 domain size from the number of
/// application-defined steps.
pub(crate) const fn total_circuit_counts(num_application_steps: usize) -> (usize, u32) {
    let total_circuits = num_application_steps + step::NUM_INTERNAL_STEPS + NUM_INTERNAL_CIRCUITS;
    let log2_circuits = total_circuits.next_power_of_two().trailing_zeros();
    (total_circuits, log2_circuits)
}

impl InternalCircuitIndex {
    /// All variants in canonical iteration order.
    pub(crate) const ALL: [Self; NUM_INTERNAL_CIRCUITS] = [
        Self::PreambleStage,
        Self::ErrorMStage,
        Self::ErrorNStage,
        Self::QueryStage,
        Self::EvalStage,
        Self::ErrorMFinalStaged,
        Self::ErrorNFinalStaged,
        Self::EvalFinalStaged,
        Self::Hashes1Circuit,
        Self::Hashes2Circuit,
        Self::PartialCollapseCircuit,
        Self::FullCollapseCircuit,
        Self::ComputeVCircuit,
    ];

    pub(crate) fn circuit_index(self) -> CircuitIndex {
        let pos = Self::ALL.iter().position(|&v| v == self).unwrap();
        CircuitIndex::from_u32(pos as u32)
    }
}

/// Per-internal-circuit storage indexed by [`InternalCircuitIndex`].
///
/// Each field corresponds 1:1 to a variant of [`InternalCircuitIndex`].
/// Use [`get`](Self::get) to look up by variant, and
/// [`from_fn`](Self::from_fn) / [`try_from_fn`](Self::try_from_fn) to
/// construct from a closure.
#[derive(Clone)]
pub(crate) struct InternalCircuitValues<T> {
    pub preamble_stage: T,
    pub error_m_stage: T,
    pub error_n_stage: T,
    pub query_stage: T,
    pub eval_stage: T,
    pub error_m_final_staged: T,
    pub error_n_final_staged: T,
    pub eval_final_staged: T,
    pub hashes_1_circuit: T,
    pub hashes_2_circuit: T,
    pub partial_collapse_circuit: T,
    pub full_collapse_circuit: T,
    pub compute_v_circuit: T,
}

impl<T> InternalCircuitValues<T> {
    /// Look up the value for the given internal circuit index.
    pub fn get(&self, id: InternalCircuitIndex) -> &T {
        use InternalCircuitIndex::*;
        match id {
            PreambleStage => &self.preamble_stage,
            ErrorMStage => &self.error_m_stage,
            ErrorNStage => &self.error_n_stage,
            QueryStage => &self.query_stage,
            EvalStage => &self.eval_stage,
            ErrorMFinalStaged => &self.error_m_final_staged,
            ErrorNFinalStaged => &self.error_n_final_staged,
            EvalFinalStaged => &self.eval_final_staged,
            Hashes1Circuit => &self.hashes_1_circuit,
            Hashes2Circuit => &self.hashes_2_circuit,
            PartialCollapseCircuit => &self.partial_collapse_circuit,
            FullCollapseCircuit => &self.full_collapse_circuit,
            ComputeVCircuit => &self.compute_v_circuit,
        }
    }

    /// Construct from a closure called once per variant in [`ALL`](InternalCircuitIndex::ALL)
    /// order.
    pub fn from_fn(mut f: impl FnMut(InternalCircuitIndex) -> T) -> Self {
        match Self::try_from_fn(|id| Ok::<_, core::convert::Infallible>(f(id))) {
            Ok(v) => v,
            Err(e) => match e {},
        }
    }

    /// Fallible construction from a closure called once per variant.
    pub fn try_from_fn<E>(
        mut f: impl FnMut(InternalCircuitIndex) -> core::result::Result<T, E>,
    ) -> core::result::Result<Self, E> {
        use InternalCircuitIndex::*;
        Ok(InternalCircuitValues {
            preamble_stage: f(PreambleStage)?,
            error_m_stage: f(ErrorMStage)?,
            error_n_stage: f(ErrorNStage)?,
            query_stage: f(QueryStage)?,
            eval_stage: f(EvalStage)?,
            error_m_final_staged: f(ErrorMFinalStaged)?,
            error_n_final_staged: f(ErrorNFinalStaged)?,
            eval_final_staged: f(EvalFinalStaged)?,
            hashes_1_circuit: f(Hashes1Circuit)?,
            hashes_2_circuit: f(Hashes2Circuit)?,
            partial_collapse_circuit: f(PartialCollapseCircuit)?,
            full_collapse_circuit: f(FullCollapseCircuit)?,
            compute_v_circuit: f(ComputeVCircuit)?,
        })
    }
}

/// Registers internal native circuits into the provided registry.
///
/// All circuits registered here are internal and will be placed
/// before any application steps.
pub(crate) fn register_all<'params, C: Cycle, R: Rank, const HEADER_SIZE: usize>(
    mut registry: RegistryBuilder<'params, C::CircuitField, R>,
    params: &'params C::Params,
    log2_circuits: u32,
) -> Result<RegistryBuilder<'params, C::CircuitField, R>> {
    let initial_internal_circuits = registry.num_internal_circuits();

    for &id in &InternalCircuitIndex::ALL {
        use InternalCircuitIndex::*;
        registry = match id {
            PreambleStage => {
                registry.register_internal_mask::<stages::preamble::Stage<C, R, HEADER_SIZE>>()?
            }
            ErrorMStage => {
                registry.register_internal_mask::<stages::error_m::Stage<C, R, HEADER_SIZE, NativeParameters>>()?
            }
            ErrorNStage => {
                registry.register_internal_mask::<stages::error_n::Stage<C, R, HEADER_SIZE, NativeParameters>>()?
            }
            QueryStage => {
                registry.register_internal_mask::<stages::query::Stage<C, R, HEADER_SIZE>>()?
            }
            EvalStage => {
                registry.register_internal_mask::<stages::eval::Stage<C, R, HEADER_SIZE>>()?
            }
            ErrorMFinalStaged => {
                registry.register_internal_final_mask::<stages::error_m::Stage<C, R, HEADER_SIZE, NativeParameters>>()?
            }
            ErrorNFinalStaged => {
                registry.register_internal_final_mask::<stages::error_n::Stage<C, R, HEADER_SIZE, NativeParameters>>()?
            }
            EvalFinalStaged => {
                registry.register_internal_final_mask::<stages::eval::Stage<C, R, HEADER_SIZE>>()?
            }
            Hashes1Circuit => {
                registry.register_internal_circuit(hashes_1::Circuit::<C, R, HEADER_SIZE, NativeParameters>::new(params, log2_circuits))?
            }
            Hashes2Circuit => {
                registry.register_internal_circuit(hashes_2::Circuit::<C, R, HEADER_SIZE, NativeParameters>::new(params))?
            }
            PartialCollapseCircuit => {
                registry.register_internal_circuit(partial_collapse::Circuit::<C, R, HEADER_SIZE, NativeParameters>::new())?
            }
            FullCollapseCircuit => {
                registry.register_internal_circuit(full_collapse::Circuit::<C, R, HEADER_SIZE, NativeParameters>::new())?
            }
            ComputeVCircuit => {
                registry.register_internal_circuit(compute_v::Circuit::<C, R, HEADER_SIZE>::new())?
            }
        };
    }

    assert_eq!(
        registry.num_internal_circuits(),
        initial_internal_circuits + NUM_INTERNAL_CIRCUITS,
        "internal circuit count mismatch"
    );

    Ok(registry)
}
