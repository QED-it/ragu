use ff::Field;
use ragu_core::maybe::Always;
use ragu_core::{
    Result,
    drivers::{Driver, DriverValue},
    gadgets::{Bound, Kind},
    maybe::Maybe,
    routines::{Prediction, Routine},
};
use ragu_pasta::Fp;
use ragu_primitives::{Element, Simulator};

use crate::{
    Circuit,
    metrics::{self, RoutineFingerprint, RoutineIdentity},
};

/// Canonical single-square routine.
#[derive(Clone)]
struct SquareOnce;

impl Routine<Fp> for SquareOnce {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        input.square(dr)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// N sequential squares — parameterized constraint count.
#[derive(Clone)]
struct SquareN<const N: usize>;

impl<const N: usize> Routine<Fp> for SquareN<N> {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let mut acc = input;
        for _ in 0..N {
            acc = acc.square(dr)?;
        }
        Ok(acc)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Identical body to `SquareOnce` but a distinct Rust type.
#[derive(Clone)]
struct SquareOnceAlias;

impl Routine<Fp> for SquareOnceAlias {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        input.square(dr)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Zero input wires — produces an Element from nothing.
#[derive(Clone)]
struct Produce;

impl Routine<Fp> for Produce {
    type Input = Kind![Fp; ()];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        _input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        Element::alloc(dr, D::just(|| Fp::ZERO))
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Two input wires — adds two elements.
#[derive(Clone)]
struct AddTwo;

impl Routine<Fp> for AddTwo {
    type Input = Kind![Fp; (Element<'_, _>, Element<'_, _>)];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let (a, b) = input;
        Ok(a.add(dr, &b))
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Duplicates input — output type differs from SquareOnce.
#[derive(Clone)]
struct Duplicate;

impl Routine<Fp> for Duplicate {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; (Element<'_, _>, Element<'_, _>)];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        Ok((input.clone(), input))
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Zero wires, zero constraints.
#[derive(Clone)]
struct EmptyRoutine;

impl Routine<Fp> for EmptyRoutine {
    type Input = Kind![Fp; ()];
    type Output = Kind![Fp; ()];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        Ok(())
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Pure delegation — calls SquareOnce as a nested routine.
#[derive(Clone)]
struct PureNesting;

impl Routine<Fp> for PureNesting {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        dr.routine(SquareOnce, input)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Nested call plus an extra constraint — fingerprint must differ from SquareOnce.
#[derive(Clone)]
struct NestingWithExtra;

impl Routine<Fp> for NestingWithExtra {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let result = dr.routine(SquareOnce, input)?;
        result.enforce_zero(dr)?;
        Ok(result)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Linear constraints only — no multiplications.
#[derive(Clone)]
struct LinearOnly;

impl Routine<Fp> for LinearOnly {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        input.enforce_zero(dr)?;
        input.enforce_zero(dr)?;
        Ok(input)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Mixed alloc + mul + enforce_zero.
#[derive(Clone)]
struct MixedConstraints;

impl Routine<Fp> for MixedConstraints {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let aux = Element::alloc(dr, D::just(|| Fp::ONE))?;
        let sq = input.square(dr)?;
        sq.enforce_zero(dr)?;
        Ok(aux)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Three-level nesting: wraps PureNesting which wraps SquareOnce.
#[derive(Clone)]
struct TripleNesting;

impl Routine<Fp> for TripleNesting {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        dr.routine(PureNesting, input)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Calls SquareOnce then squares the result locally.
#[derive(Clone)]
struct NestThenSquare;

impl Routine<Fp> for NestThenSquare {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let nested = dr.routine(SquareOnce, input)?;
        nested.square(dr)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Calls SquareOnce then adds the result to itself locally.
#[derive(Clone)]
struct NestThenAdd;

impl Routine<Fp> for NestThenAdd {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let nested = dr.routine(SquareOnce, input)?;
        Ok(nested.add(dr, &nested))
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Delegates to SquareOnce then enforces output == 0.
#[derive(Clone)]
struct DelegateThenEnforce;

impl Routine<Fp> for DelegateThenEnforce {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let output = dr.routine(SquareOnce, input)?;
        output.enforce_zero(dr)?;
        Ok(output)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Allocates two fresh wires, enforces the second == 0. No delegation.
///
/// When run through `fingerprint_routine` (which does not clear `available_b`
/// after input remapping), the first alloc drains the stale b-wire (value 3)
/// without consuming a mul gate, so the second alloc starts a fresh gate at a=4
/// — the same position the Counter assigns to a remapped output wire.  This is
/// the `available_b` leak expressing itself as a Tier 1 alias.
#[derive(Clone)]
struct AllocThenEnforce;

impl Routine<Fp> for AllocThenEnforce {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        _input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let _consume_paired_b = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        let fresh = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        fresh.enforce_zero(dr)?;
        Ok(fresh)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Allocates a fresh wire and returns it. No constraints.
///
/// As with [`AllocThenEnforce`], the first alloc consumes the paired
/// b-wire so that the returned wire comes from a fresh gate.
#[derive(Clone)]
struct AllocOnly;

impl Routine<Fp> for AllocOnly {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        _input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let _consume_paired_b = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        Element::alloc(dr, D::just(|| Fp::ZERO))
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Delegates to SquareOnce, adds output + input, enforces sum == 0.
#[derive(Clone)]
struct DelegateThenAddEnforce;

impl Routine<Fp> for DelegateThenAddEnforce {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let output = dr.routine(SquareOnce, input.clone())?;
        let sum = output.add(dr, &input);
        sum.enforce_zero(dr)?;
        Ok(output)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Allocates two fresh wires, adds the second + input, enforces sum == 0.
#[derive(Clone)]
struct AllocThenAddEnforce;

impl Routine<Fp> for AllocThenAddEnforce {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let _consume_paired_b = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        let fresh = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        let sum = fresh.add(dr, &input);
        sum.enforce_zero(dr)?;
        Ok(fresh)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Like SquareOnce but preceded by two trivial enforce_zero calls (empty linear
/// combination). The $s(X, Y)$ polynomial differs because the real constraints
/// land at $Y^2$ and $Y^3$ instead of $Y^0$ and $Y^1$, but the Horner scalar is
/// the same because the leading contributions are zero.
#[derive(Clone)]
struct SquareOnceWithLeadingTrivial;

impl Routine<Fp> for SquareOnceWithLeadingTrivial {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        dr.enforce_zero(|lc| lc)?;
        dr.enforce_zero(|lc| lc)?;
        input.square(dr)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Delegates to SquareOnce, pads with an extra alloc (to match constraint
/// counts with non-delegating routines), enforces the CHILD OUTPUT wire == 0.
#[derive(Clone)]
struct DelegateEnforceChild;

impl Routine<Fp> for DelegateEnforceChild {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let output = dr.routine(SquareOnce, input)?;
        let _consume_b = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        let _pad = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        output.enforce_zero(dr)?;
        Ok(output)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Delegates to SquareOnce, pads with an extra alloc (to match constraint
/// counts with non-delegating routines), enforces the LOCAL ALLOC wire == 0.
#[derive(Clone)]
struct DelegateEnforceLocal;

impl Routine<Fp> for DelegateEnforceLocal {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let output = dr.routine(SquareOnce, input)?;
        let _consume_b = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        let fresh = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        fresh.enforce_zero(dr)?;
        Ok(output)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Delegates to SquareOnce, pads with one alloc (to occupy a mul gate),
/// enforces the CHILD OUTPUT wire == 0.
///
/// Paired with [`DelegateAllocEnforceFirst`]: both have one local mul
/// gate and one linear constraint after delegation, but one enforces the
/// child output while the other enforces the local allocation.
#[derive(Clone)]
struct DelegatePadEnforceOutput;

impl Routine<Fp> for DelegatePadEnforceOutput {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let output = dr.routine(SquareOnce, input)?;
        let _pad = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        output.enforce_zero(dr)?;
        Ok(output)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

/// Delegates to SquareOnce, allocates one wire, enforces the LOCAL ALLOC
/// wire == 0.
///
/// Paired with [`DelegatePadEnforceOutput`]: both have one local mul
/// gate and one linear constraint, but this routine enforces a fresh
/// input-independent allocation instead of the child's input-dependent
/// output.
#[derive(Clone)]
struct DelegateAllocEnforceFirst;

impl Routine<Fp> for DelegateAllocEnforceFirst {
    type Input = Kind![Fp; Element<'_, _>];
    type Output = Kind![Fp; Element<'_, _>];
    type Aux<'dr> = ();

    fn execute<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        input: Bound<'dr, D, Self::Input>,
        _aux: DriverValue<D, Self::Aux<'dr>>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        let output = dr.routine(SquareOnce, input)?;
        let local = Element::alloc(dr, D::just(|| Fp::ZERO))?;
        local.enforce_zero(dr)?;
        Ok(output)
    }

    fn predict<'dr, D: Driver<'dr, F = Fp>>(
        &self,
        _dr: &mut D,
        _input: &Bound<'dr, D, Self::Input>,
    ) -> Result<Prediction<Bound<'dr, D, Self::Output>, DriverValue<D, Self::Aux<'dr>>>> {
        Ok(Prediction::Unknown(D::just(|| ())))
    }
}

fn fingerprint_elem(
    routine: &impl Routine<Fp, Input = Kind![Fp; Element<'_, _>]>,
) -> RoutineFingerprint {
    let mut sim = Simulator::<Fp>::new();
    let input = Element::alloc(&mut sim, Always::<Fp>::just(|| Fp::ONE)).unwrap();
    match metrics::fingerprint_routine::<Fp, Simulator<Fp>, _>(routine, &input).unwrap() {
        RoutineIdentity::Routine(fp) => fp,
        RoutineIdentity::Root => panic!("expected Routine variant"),
    }
}

fn fingerprint_unit(routine: &impl Routine<Fp, Input = Kind![Fp; ()]>) -> RoutineFingerprint {
    match metrics::fingerprint_routine::<Fp, Simulator<Fp>, _>(routine, &()).unwrap() {
        RoutineIdentity::Routine(fp) => fp,
        RoutineIdentity::Root => panic!("expected Routine variant"),
    }
}

fn fingerprint_pair(
    routine: &impl Routine<Fp, Input = Kind![Fp; (Element<'_, _>, Element<'_, _>)]>,
) -> RoutineFingerprint {
    let sim = &mut Simulator::<Fp>::new();
    let a = Element::alloc(sim, Always::<Fp>::just(|| Fp::ONE)).unwrap();
    let b = Element::alloc(sim, Always::<Fp>::just(|| Fp::ONE)).unwrap();
    match metrics::fingerprint_routine::<Fp, Simulator<Fp>, _>(routine, &(a, b)).unwrap() {
        RoutineIdentity::Routine(fp) => fp,
        RoutineIdentity::Root => panic!("expected Routine variant"),
    }
}

/// Extracts a routine's fingerprint via `metrics::eval`, which runs
/// through `Counter::routine` (the production path that correctly clears
/// `available_b` after input remapping).
fn fingerprint_via_eval<Ro>(routine: &Ro) -> RoutineFingerprint
where
    Ro: Routine<Fp, Input = Kind![Fp; Element<'_, _>], Output = Kind![Fp; Element<'_, _>]>
        + Clone
        + Send
        + Sync,
    for<'dr> Ro::Aux<'dr>: Send + Clone,
{
    let m = metrics::eval(&SingleRoutineCircuit(routine.clone())).unwrap();
    assert!(
        m.segments.len() >= 2,
        "fingerprint_via_eval expects at least 2 segments (root + routine); \
         got {}",
        m.segments.len(),
    );
    match m.segments[1].identity {
        RoutineIdentity::Routine(fp) => fp,
        RoutineIdentity::Root => panic!("expected Routine variant"),
    }
}

/// Wraps an `Element → Element` routine as a minimal Circuit for metrics tests.
#[derive(Clone)]
struct SingleRoutineCircuit<Ro: Clone>(Ro);

impl<Ro> Circuit<Fp> for SingleRoutineCircuit<Ro>
where
    Ro: Routine<Fp, Input = Kind![Fp; Element<'_, _>], Output = Kind![Fp; Element<'_, _>]>
        + Clone
        + Send
        + Sync,
    for<'dr> Ro::Aux<'dr>: Send + Clone,
{
    type Instance<'source> = Fp;
    type Output = Kind![Fp; Element<'_, _>];
    type Witness<'source> = Fp;
    type Aux<'source> = ();

    fn instance<'dr, 'source: 'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        instance: DriverValue<D, Self::Instance<'source>>,
    ) -> Result<Bound<'dr, D, Self::Output>>
    where
        Self: 'dr,
    {
        Element::alloc(dr, instance)
    }

    fn witness<'dr, 'source: 'dr, D: Driver<'dr, F = Fp>>(
        &self,
        dr: &mut D,
        witness: DriverValue<D, Self::Witness<'source>>,
    ) -> Result<(
        Bound<'dr, D, Self::Output>,
        DriverValue<D, Self::Aux<'source>>,
    )>
    where
        Self: 'dr,
    {
        let input = Element::alloc(dr, witness)?;
        let output = dr.routine(self.0.clone(), input)?;
        Ok((output, D::just(|| ())))
    }
}

/// Same routine fingerprinted twice produces identical results.
#[test]
fn test_determinism() {
    assert_eq!(fingerprint_elem(&SquareOnce), fingerprint_elem(&SquareOnce));
    assert_eq!(
        fingerprint_unit(&EmptyRoutine),
        fingerprint_unit(&EmptyRoutine)
    );
}

/// Distinct Rust types with identical constraint structure share a fingerprint.
#[test]
fn test_structural_equivalence() {
    let sq = fingerprint_elem(&SquareOnce);
    let alias = fingerprint_elem(&SquareOnceAlias);
    let n1 = fingerprint_elem(&SquareN::<1>);

    assert_eq!(sq, alias);
    assert_eq!(sq, n1);
    assert_eq!(alias, n1);
}

/// Different constraint counts produce different fingerprints.
#[test]
fn test_structural_sensitivity() {
    let n1 = fingerprint_elem(&SquareN::<1>);
    let n2 = fingerprint_elem(&SquareN::<2>);
    let n3 = fingerprint_elem(&SquareN::<3>);

    assert_ne!(n1, n2);
    assert_ne!(n2, n3);
    assert_ne!(n1, n3);
}

/// Routines with different Input/Output TypeIds are always distinct.
#[test]
fn test_type_discrimination() {
    let all = [
        fingerprint_elem(&SquareOnce),
        fingerprint_unit(&Produce),
        fingerprint_elem(&Duplicate),
        fingerprint_unit(&EmptyRoutine),
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(all[i], all[j], "routines {i} and {j} must be distinct");
        }
    }
}

/// Different input wire counts produce different fingerprints.
#[test]
fn test_input_wire_count() {
    assert_ne!(fingerprint_elem(&SquareOnce), fingerprint_pair(&AddTwo));
}

/// Nested routine calls produce fingerprints based on local constraints only.
#[test]
fn test_nesting() {
    let square = fingerprint_elem(&SquareOnce);
    let pure = fingerprint_elem(&PureNesting);
    let extra = fingerprint_elem(&NestingWithExtra);

    assert_ne!(square, pure);
    assert_ne!(square, extra);
    assert_ne!(pure, extra);
}

/// Zero-constraint routines are distinguished by TypeId pairs alone.
#[test]
fn test_degenerate_cases() {
    assert_ne!(fingerprint_unit(&EmptyRoutine), fingerprint_unit(&Produce));
    assert_ne!(fingerprint_elem(&Duplicate), fingerprint_elem(&SquareOnce));
}

/// Segment 0 is Root; segments 1+ are Routine.
#[test]
fn test_root_identity() {
    let metrics = metrics::eval(&SingleRoutineCircuit(SquareOnce)).unwrap();

    assert_eq!(metrics.segments.len(), 2);
    assert!(matches!(
        metrics.segments[0].identity,
        RoutineIdentity::Root
    ));
    assert!(matches!(
        metrics.segments[1].identity,
        RoutineIdentity::Routine(_)
    ));
}

/// Guard against accidental changes to the fingerprint computation.
#[test]
fn test_known_value_regression() {
    assert_eq!(fingerprint_elem(&SquareOnce).scalar(), 21);
}

/// Fingerprint from metrics::eval matches standalone fingerprint_routine.
#[test]
fn test_metrics_integration() {
    let metrics = metrics::eval(&SingleRoutineCircuit(SquareOnce)).unwrap();
    let direct = fingerprint_elem(&SquareOnce);

    match metrics.segments[1].identity {
        RoutineIdentity::Routine(fp) => assert_eq!(fp, direct),
        RoutineIdentity::Root => panic!("record 1 should be Routine"),
    }
    assert!(metrics.segments[1].num_multiplication_constraints > 0);
}

/// Routines with only linear constraints (no multiplications) get nonzero fingerprints.
#[test]
fn test_linear_only() {
    let linear = fingerprint_elem(&LinearOnly);
    assert_ne!(linear, fingerprint_elem(&SquareOnce));
    assert_ne!(linear.scalar(), 0);
    assert_ne!(linear, fingerprint_unit(&EmptyRoutine));
}

/// Mixed alloc + mul + enforce_zero produces a fingerprint distinct from pure mul or pure linear.
#[test]
fn test_mixed_constraints() {
    let mixed = fingerprint_elem(&MixedConstraints);
    assert_ne!(mixed, fingerprint_elem(&SquareOnce));
    assert_ne!(mixed, fingerprint_elem(&LinearOnly));
}

/// Pure delegation wrappers are nesting-depth-invariant; metrics produces correct segment count.
#[test]
fn test_triple_nesting() {
    let triple = fingerprint_elem(&TripleNesting);
    assert_eq!(triple, fingerprint_elem(&PureNesting));
    assert_ne!(triple, fingerprint_elem(&SquareOnce));

    let metrics = metrics::eval(&SingleRoutineCircuit(TripleNesting)).unwrap();
    assert_eq!(metrics.segments.len(), 4);
}

/// Parents that call the same child but differ in post-processing get different fingerprints.
#[test]
fn test_output_remapping_preserves_parent() {
    assert_ne!(
        fingerprint_elem(&NestThenSquare),
        fingerprint_elem(&NestThenAdd)
    );
}

// ── Regression tests: fingerprint aliasing bugs ─────────────────────
//
// Each diagram traces a routine through the Counter driver with fixed
// constants x0=2, x1=3, x2=5, y=7.  After input remapping, every
// Elem→Elem routine starts from the same parent state:
//
//     in=2  │  a→4  b→9  c→25  paired-b=3  scalar=0
//
// Notation:
//
//     in → 2              input wire gets Counter value 2
//     ──( Child )──       delegate to child (fingerprinted separately)
//     out:4 ← remap       output wire via uncounted save/restore alloc
//     w:4 ← alloc         counted mul gate; wire gets current_a = 4
//     _:3 ← paired        consume leftover b-wire from prior gate
//     w:6 ← add(a, b)     linear combination with value 6
//     enforce(w:4)         enforce_zero on wire: s = s*7 + 4
//     enforce(empty)       empty LC: s = s*7 + 0
//     s=N  M*mul  L*lin    final scalar, mul-gate count, linear count
//
// ── Tier 1: missing constraint counts ───────────────────────────────
//
// The fingerprint tuple (TypeId(Input), TypeId(Output), scalar) only
// accumulates from enforce_zero calls.  Mul-gate counts and linear
// constraint counts are absent, so routines that differ only in those
// dimensions produce identical fingerprints.

/// ```text
///  DelegateThenEnforce          AllocThenEnforce
///  ───────────────────          ────────────────
///  in → 2                       in → 2
///  ──( SquareOnce )──            _:3 ← paired
///  out:4 ← remap                 f:4 ← alloc
///  enforce(out:4)                enforce(f:4)
///    s = 0*7 + 4 = 4              s = 0*7 + 4 = 4
///  ─── s=4  0*mul  1*lin ───   ─── s=4  1*mul  1*lin ───
///            ^^^^^                      ^^^^^
/// ```
///
/// Bug: `fingerprint_routine` does not clear `available_b` after input
/// remapping, so `_consume_paired_b` drains the stale b-wire without
/// consuming a mul gate.  The subsequent `fresh = alloc()` therefore
/// starts at the same geometric position (a=4) as the remapped output
/// wire, producing the same Horner contribution.  This aliasing is an
/// artifact of the `available_b` leak; the mul-gate count also differs
/// (0 vs 1) but fixing counts alone would resolve it too.
#[test]
fn test_aliasing_delegate_vs_alloc_enforce() {
    assert_ne!(
        fingerprint_elem(&DelegateThenEnforce),
        fingerprint_elem(&AllocThenEnforce),
    );
}

/// ```text
///  PureNesting                  AllocOnly
///  ───────────                  ─────────
///  in → 2                       in → 2
///  ──( SquareOnce )──            _:3 ← paired
///  out:4 ← remap                 f:4 ← alloc
///  (no constraints)              (no constraints)
///  ─── s=0  0*mul  0*lin ───   ─── s=0  1*mul  0*lin ───
///            ^^^^^                      ^^^^^
/// ```
///
/// Bug: with no enforce_zero calls both scalars are trivially 0.
/// The routines differ by a whole mul gate — different $s(X, Y)$
/// polynomials — but the fingerprint can't see it.
#[test]
fn test_aliasing_delegate_vs_alloc_no_constraints() {
    assert_ne!(fingerprint_elem(&PureNesting), fingerprint_elem(&AllocOnly),);
}

/// ```text
///  DelegateThenAddEnforce       AllocThenAddEnforce
///  ──────────────────────       ───────────────────
///  in → 2                       in → 2
///  ──( SquareOnce )──            _:3 ← paired
///  out:4 ← remap                 f:4 ← alloc
///  sum:6 ← add(out:4, in:2)     sum:6 ← add(f:4, in:2)
///  enforce(sum:6)                enforce(sum:6)
///    s = 0*7 + 6 = 6              s = 0*7 + 6 = 6
///  ─── s=6  0*mul  1*lin ───   ─── s=6  1*mul  1*lin ───
///            ^^^^^                      ^^^^^
/// ```
///
/// Bug: aliasing propagates through linear combinations.  The values
/// `out:4` and `f:4` are identical because the `available_b` leak
/// places the `fresh` alloc at a=4 (same position as the remapped
/// output wire).  If `available_b` were cleared, `f` would be 9 and
/// the sums would differ.  The mul-gate count also differs (0 vs 1)
/// but that is a secondary issue here.
#[test]
fn test_aliasing_propagates_through_linear_combinations() {
    assert_ne!(
        fingerprint_elem(&DelegateThenAddEnforce),
        fingerprint_elem(&AllocThenAddEnforce),
    );
}

/// ```text
///  SquareOnce                   SquareOnceWithLeadingTrivial
///  ──────────                   ────────────────────────────
///  in → 2                       in → 2
///                                enforce(empty) → +0  (vanishes!)
///                                enforce(empty) → +0  (vanishes!)
///  square(in):                   square(in):
///    gate: a=4 b=9                 gate: a=4 b=9
///    enforce(a-in = 2)             enforce(a-in = 2)
///      s = 0*7 + 2 = 2              s = 0*7 + 2 = 2
///    enforce(b-in = 7)             enforce(b-in = 7)
///      s = 2*7 + 7 = 21             s = 2*7 + 7 = 21
///  ─── s=21  1*mul  2*lin ──   ─── s=21  1*mul  4*lin ──
///                    ^^^^^                       ^^^^^
/// ```
///
/// Bug: leading enforce_zero on empty LCs contribute 0 to the Horner
/// accumulator, so they vanish: $0 \cdot 7^k = 0$.  The linear constraint
/// count differs (2 vs 4) but isn't in the fingerprint.
#[test]
fn test_aliasing_leading_trivial_linear_constraints() {
    assert_ne!(
        fingerprint_elem(&SquareOnce),
        fingerprint_elem(&SquareOnceWithLeadingTrivial),
    );
}

/// The aliased routine pairs have genuinely different constraint structure,
/// confirmed by differing metrics (segment counts, child routines).
#[test]
fn test_aliasing_metrics_confirm_different_structure() {
    let m1 = metrics::eval(&SingleRoutineCircuit(DelegateThenEnforce)).unwrap();
    let m2 = metrics::eval(&SingleRoutineCircuit(AllocThenEnforce)).unwrap();

    // DelegateThenEnforce has a child segment (SquareOnce), AllocThenEnforce does not.
    assert_ne!(m1.segments.len(), m2.segments.len());

    let m3 = metrics::eval(&SingleRoutineCircuit(PureNesting)).unwrap();
    let m4 = metrics::eval(&SingleRoutineCircuit(AllocOnly)).unwrap();

    // PureNesting has a child segment, AllocOnly does not.
    assert_ne!(m3.segments.len(), m4.segments.len());
}

// ── available_b leak amplification ───────────────────────────────
//
// The following tests were originally designed to demonstrate a
// save/restore wire-value collision, but the aliasing they exhibit
// is actually caused by the `available_b` leak in `fingerprint_routine`
// (documented in the Tier 3 section below).  Via `metrics::eval` —
// which correctly clears `available_b` — these pairs produce different
// scalars and are NOT aliased.
//
// The genuine save/restore collision (which persists through `eval`)
// is demonstrated separately by DelegatePadEnforceOutput vs
// DelegateAllocEnforceFirst in the "Wire collision via eval" section.

/// ```text
///  DelegateEnforceChild         DelegateEnforceLocal
///  ────────────────────         ────────────────────
///  in → 2                       in → 2
///  ──( SquareOnce )──           ──( SquareOnce )──
///  out:4 ← remap                out:4 ← remap
///  _:3 ← paired (consume_b)     _:3 ← paired (consume_b)
///  _:4 ← alloc (pad)           loc:4 ← alloc
///                                         ↑
///  enforce(out:4)               enforce(loc:4)
///         ↑                        ↑
///    child output               local alloc ── SAME VALUE
///    s = 0*7 + 4 = 4              s = 0*7 + 4 = 4
///  ─── s=4  1*mul  1*lin ───   ─── s=4  1*mul  1*lin ───
/// ```
///
/// Bug: both wires land at a=4 because `fingerprint_routine` leaks
/// `available_b`, causing `_consume_b` to drain the stale b-wire
/// without consuming a mul gate.  Via `metrics::eval` (which clears
/// `available_b`), these produce different scalars (s=4 vs s=9) and
/// are NOT aliased.  Fixing the `available_b` leak alone would make
/// this test pass.
#[test]
fn test_wire_collision_child_output_vs_local_alloc() {
    assert_ne!(
        fingerprint_elem(&DelegateEnforceChild),
        fingerprint_elem(&DelegateEnforceLocal),
    );
}

/// ```text
///  DelegateEnforceChild         AllocThenEnforce
///  ────────────────────         ────────────────
///  in → 2                       in → 2
///  ──( SquareOnce )──            _:3 ← paired
///  out:4 ← remap                 f:4 ← alloc
///  _:3 ← paired                 enforce(f:4)
///  _:4 ← alloc                    s = 0*7 + 4 = 4
///  enforce(out:4)               ─── s=4  1*mul  1*lin ───
///    s = 0*7 + 4 = 4
///  ─── s=4  1*mul  1*lin ───      f is input-INDEPENDENT
///                                 (fresh alloc, ignores in)
///    out is input-DEPENDENT
///    (child computed from in)
/// ```
///
/// Bug: left enforces a child-output wire that varies with the
/// input; right enforces a local alloc that doesn't.  The aliasing
/// here is caused by the `available_b` leak in `fingerprint_routine`
/// (same mechanism as the test above).  Via `eval`, these produce
/// different scalars (s=4 vs s=9) and are not aliased.
#[test]
fn test_delegation_indistinguishable_from_alloc_with_matched_counts() {
    assert_ne!(
        fingerprint_elem(&DelegateEnforceChild),
        fingerprint_elem(&AllocThenEnforce),
    );
}

/// This pair has identical segment structure AND identical per-segment
/// constraint counts.  The aliasing through `fingerprint_elem` is caused
/// by the `available_b` leak, not by save/restore wire collision — fixing
/// the leak alone would make `test_wire_collision_child_output_vs_local_alloc`
/// pass without any constraint-count changes.
#[test]
fn test_wire_collision_metrics_identical() {
    let m1 = metrics::eval(&SingleRoutineCircuit(DelegateEnforceChild)).unwrap();
    let m2 = metrics::eval(&SingleRoutineCircuit(DelegateEnforceLocal)).unwrap();

    // Same segment count (root + parent + SquareOnce child).
    assert_eq!(m1.segments.len(), m2.segments.len());
    assert_eq!(m1.segments.len(), 3);

    // Same constraint counts in every segment.
    for (s1, s2) in m1.segments.iter().zip(m2.segments.iter()) {
        assert_eq!(
            s1.num_multiplication_constraints,
            s2.num_multiplication_constraints
        );
        assert_eq!(s1.num_linear_constraints, s2.num_linear_constraints);
    }
}

// ── fingerprint_routine bug: available_b leak ───────────────────────
//
// The standalone `fingerprint_routine` function does not clear
// `available_b` after input remapping, while `Counter::routine`
// (used by `eval`) does (metrics.rs line 378).  For routines whose
// first driver operation is `alloc`, the stale b-wire from the
// input gate changes subsequent wire values and thus the Horner
// scalar.
//
// This inconsistency means `fingerprint_routine` can disagree with
// `eval` for the same routine, AND it amplifies several of the
// aliasing tests above — four of the six Tier 1/2 test failures
// are artifacts of this leak, not of the missing-count or
// wire-collision design issues.

/// `fingerprint_routine` (standalone) and `fingerprint_via_eval`
/// (production path through `Counter::routine`) must agree for every
/// `Element → Element` test routine.
#[test]
fn test_cross_path_consistency() {
    macro_rules! check {
        ($($routine:expr),+ $(,)?) => {
            $(assert_eq!(
                fingerprint_elem(&$routine),
                fingerprint_via_eval(&$routine),
                concat!("cross-path mismatch for ", stringify!($routine)),
            );)+
        };
    }
    check![
        SquareOnce,
        SquareOnceAlias,
        PureNesting,
        NestingWithExtra,
        LinearOnly,
        MixedConstraints,
        TripleNesting,
        NestThenSquare,
        NestThenAdd,
        DelegateThenEnforce,
        AllocThenEnforce,
        AllocOnly,
        DelegateThenAddEnforce,
        AllocThenAddEnforce,
        SquareOnceWithLeadingTrivial,
        DelegateEnforceChild,
        DelegateEnforceLocal,
        DelegatePadEnforceOutput,
        DelegateAllocEnforceFirst,
    ];
}

/// The two genuine design bugs persist through `eval`: missing
/// constraint counts (both scalars trivially 0) and vanishing empty
/// linear combinations (leading $0 \cdot y^k$ terms disappear).
#[test]
fn test_missing_counts_via_eval() {
    // PureNesting vs AllocOnly: no enforce_zero → both scalar 0.
    // Different mul-gate counts (0 vs 1) but not in the fingerprint.
    assert_ne!(
        fingerprint_via_eval(&PureNesting),
        fingerprint_via_eval(&AllocOnly),
    );
}

#[test]
fn test_vanishing_leading_trivial_via_eval() {
    // SquareOnce vs SquareOnceWithLeadingTrivial: leading empty
    // enforce_zero calls contribute 0 to Horner, so both scalar 21.
    // Different linear counts (2 vs 4) but not in the fingerprint.
    assert_ne!(
        fingerprint_via_eval(&SquareOnce),
        fingerprint_via_eval(&SquareOnceWithLeadingTrivial),
    );
}

// ── Wire collision via eval (save/restore) ──────────────────────────
//
// Even after fixing the available_b leak AND adding constraint counts,
// the output remapping's save/restore still causes a collision: the
// remap wire and the first local alloc share the same geometric
// position.  This is because save/restore rewinds current_a to the
// same value used for the remap alloc, so the next counted mul gate
// returns the same a-wire value.
//
// The following pair demonstrates this through `eval`, with matching
// constraint counts in every segment — proving that adding counts
// alone cannot fix it.

/// ```text
///  DelegatePadEnforceOutput     DelegateAllocEnforceFirst
///  ────────────────────────     ─────────────────────────
///  in → 2                       in → 2
///  ──( SquareOnce )──           ──( SquareOnce )──
///  out:4 ← remap                out:4 ← remap
///  pad:4 ← alloc                loc:4 ← alloc
///           ↑                            ↑
///  enforce(out:4)               enforce(loc:4)
///          ↑                             ↑
///    child output               local alloc ── SAME VALUE
///    s = 0*7 + 4 = 4              s = 0*7 + 4 = 4
///  ─── s=4  1*mul  1*lin ───   ─── s=4  1*mul  1*lin ───
/// ```
///
/// Bug: save/restore rewinds the geometric sequence so the output
/// remap alloc and the first counted alloc both land at a=4.  The
/// enforced wires are semantically different (child output is
/// input-dependent, local alloc is not) but evaluate identically.
/// Adding constraint counts would not help — they already match.
#[test]
fn test_wire_collision_via_eval() {
    assert_ne!(
        fingerprint_via_eval(&DelegatePadEnforceOutput),
        fingerprint_via_eval(&DelegateAllocEnforceFirst),
    );
}

/// The wire-collision pair has identical segment structure AND identical
/// per-segment constraint counts through `eval`.  This confirms the
/// collision is purely from geometric sequence aliasing, not from
/// missing count information.
#[test]
fn test_wire_collision_via_eval_metrics_identical() {
    let m1 = metrics::eval(&SingleRoutineCircuit(DelegatePadEnforceOutput)).unwrap();
    let m2 = metrics::eval(&SingleRoutineCircuit(DelegateAllocEnforceFirst)).unwrap();

    // Same segment count (root + parent + SquareOnce child).
    assert_eq!(m1.segments.len(), m2.segments.len());
    assert_eq!(m1.segments.len(), 3);

    // Same constraint counts in every segment.
    for (s1, s2) in m1.segments.iter().zip(m2.segments.iter()) {
        assert_eq!(
            s1.num_multiplication_constraints,
            s2.num_multiplication_constraints
        );
        assert_eq!(s1.num_linear_constraints, s2.num_linear_constraints);
    }
}
