//! Driver for executing circuit code natively with minimal overhead.
//!
//! ## Overview
//!
//! Circuit code is written with the [`Driver`] abstraction, which is used to
//! express operations such as allocating wires and enforcing constraints
//! alongside the corresponding witness generation logic. The simplest driver
//! would be one that simply executes circuit code directly _without_ enforcing
//! constraints; that is the purpose of this module's [`Emulator`].
//!
//! The [`Emulator`] driver never checks multiplication or linear constraints,
//! but it _can_ be used to collect and compute wire assignments.
//! When instantiated in [`Wireless`] mode, the emulator simply executes the
//! circuit code natively without wires (i.e., `Wire=()`), saving memory.
//! Whereas in [`Wired`] mode, the emulator tracks wire assignments which can
//! be extracted afterwards.
//!
//! The [`Wireless`] mode is parameterized by a [`MaybeKind`] to indicate
//! witness availability:
//!
//! * `Wireless<Empty, F>`: used mostly for wire counting and other static
//!   structure analyses. Driver still executes natively, but with `Empty`
//!   witness. Constructed via [`Emulator::counter`].
//! * `Wireless<Always<()>, F>`: used for native witness execution/generation,
//!   constructed via [`Emulator::execute`] or directly execute the logic with
//!   [`Emulator::emulate_wireless`].
//!
//! The [`Wired`] mode always has witness availability (i.e., `Always<()>`):
//!
//! * `Wired<F>`: used for native execution with wire extraction. Constructed
//!   via [`Emulator::extractor`] or directly execute the logic with
//!   [`Emulator::emulate_wired`].
//!
//! Sometimes, witness availability depends on other drivers' behavior, such as
//! when invoking an [`Emulator`] within generic circuit code itself. In such
//! cases, [`Emulator::wireless`] can be used to create wireless emulators
//! parameterized by [`MaybeKind`].
//!
//! ### Wire Extraction
//!
//! One of the common uses of an [`Emulator`] instantiated in [`Wired`] mode is
//! for computing the expected wire assignments for a [`Gadget`] after executing
//! a [`Routine`] or some other circuit code.
//!
//! ### Routines
//!
//! [`Emulator`]s are used for _natively_ executing code, not enforcing
//! correctness. As such, they short-circuit execution of [`Routine`]s using
//! [routine prediction](Routine::predict) when possible.
//!
//! ## Usage
//!
//! The [`Emulator`] can be instantiated in [`Wired`] mode using
//! [`Emulator::extractor`], and in [`Wireless`] mode using
//! [`Emulator::wireless`], [`Emulator::counter`], or [`Emulator::execute`].
//!
//! Common constructor methods:
//! * [`Emulator::extractor`] creates a wired [`Emulator`] for extracting wire
//!   assignments from a gadget.
//! * [`Emulator::execute`] creates a wireless [`Emulator`] for native witness
//!   execution/generation. This is the common case of executing circuit code
//!   natively.
//! * [`Emulator::counter`] creates a wireless [`Emulator`] for wire counting
//!   and static analysis without witness data.
//!
//! In [`Wired`] mode, wire assignments can be extracted from a gadget using
//! [`Emulator::wires`], which returns a `Vec<F>` of field elements.

use ff::Field;

use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::{
    Result,
    convert::{StripWires, WireMap},
    drivers::{Coeff, DirectSum, Driver, DriverTypes, DriverValue, LinearExpression},
    gadgets::{Bound, Gadget, GadgetKind},
    maybe::{Always, Empty, MaybeKind, Perhaps},
    routines::{Prediction, Routine},
};

/// Mode that an [`Emulator`] may be running in; usually either [`Wired`] or
/// [`Wireless`].
pub trait Mode {
    /// Equal to the resulting [`Emulator`]'s [`DriverTypes::MaybeKind`].
    type MaybeKind: MaybeKind;

    /// Equal to the resulting [`Emulator`]'s [`DriverTypes::ImplField`].
    type F: Field;

    /// Equal to the resulting [`Emulator`]'s [`DriverTypes::ImplWire`].
    type Wire: Clone;

    /// Equal to the resulting [`Emulator`]'s [`DriverTypes::LCadd`].
    type LCadd: LinearExpression<Self::Wire, Self::F>;

    /// Equal to the resulting [`Emulator`]'s [`DriverTypes::LCenforce`].
    type LCenforce: LinearExpression<Self::Wire, Self::F>;
}

/// Mode for an [`Emulator`] that tracks wire assignments.
///
/// Wired mode always has witness availability (i.e., `MaybeKind = Always<()>`).
pub struct Wired<F: Field>(PhantomData<F>);

impl<F: Field> Mode for Wired<F> {
    type MaybeKind = Always<()>;
    type F = F;
    type Wire = F;
    type LCadd = DirectSum<F>;
    type LCenforce = DirectSum<F>;
}

/// Mode for an [`Emulator`] that does not track wire assignments.
pub struct Wireless<M: MaybeKind, F: Field>(PhantomData<(M, F)>);

impl<M: MaybeKind, F: Field> Mode for Wireless<M, F> {
    type MaybeKind = M;
    type F = F;
    type Wire = ();
    type LCadd = ();
    type LCenforce = ();
}

/// A driver used to natively execute circuit code without enforcing
/// constraints. This driver also short-circuit [`Routine`] execution using
/// their provided [`Routine::predict`] method when possible.
///
/// See the [module level documentation](self) for more information.
///
/// ## [`Mode`]
///
/// The [`Emulator`] driver is parameterized on a [`Mode`], which determines
/// whether wire assignments are tracked or not ([`Wired`] vs. [`Wireless`]).
pub struct Emulator<M: Mode>(PhantomData<M>);

impl<F: Field> Emulator<Wired<F>> {
    /// Extract the wires from a gadget produced using a wired [`Emulator`].
    /// This method returns the actual wire assignments if successful.
    pub fn wires<'dr, G: Gadget<'dr, Self>>(&self, gadget: &G) -> Result<Vec<F>> {
        /// A conversion utility for extracting wire values.
        struct WireExtractor<F: Field> {
            wires: Vec<F>,
        }

        impl<F: Field> WireMap<F> for WireExtractor<F> {
            type Src = Emulator<Wired<F>>;
            type Dst = PhantomData<F>;

            fn convert_wire(&mut self, wire: &F) -> Result<()> {
                self.wires.push(*wire);
                Ok(())
            }
        }

        let mut collector = WireExtractor { wires: Vec::new() };
        <G::Kind as GadgetKind<F>>::map_gadget(gadget, &mut collector)?;
        Ok(collector.wires)
    }

    /// Creates a new [`Emulator`] driver in [`Wired`] mode for executing with
    /// a known witness.
    ///
    /// This is useful for extracting wire assignments from a [`Gadget`] using
    /// [`Emulator::wires`].
    pub fn extractor() -> Self {
        Emulator(PhantomData)
    }

    /// Helper utility for executing a closure with a freshly created wired
    /// [`Emulator`] when a witness is expected to exist.
    pub fn emulate_wired<R, W: Send>(
        witness: W,
        f: impl FnOnce(&mut Self, Always<W>) -> Result<R>,
    ) -> Result<R> {
        let mut dr = Self::extractor();
        dr.with(witness, f)
    }
}

impl<M: MaybeKind, F: Field> Emulator<Wireless<M, F>> {
    /// Creates a new [`Emulator`] driver in [`Wireless`] mode, parameterized on
    /// the existence of a witness.
    pub fn wireless() -> Self {
        Emulator(PhantomData)
    }

    /// Runs [`Routine::predict`] on a fresh wireless emulator, converting the
    /// input gadget from the source driver automatically via [`StripWires`].
    ///
    /// The source driver `D` must share the same [`MaybeKind`] as this emulator
    /// so that witness availability is preserved across the conversion. Unlike
    /// calling [`Routine::predict`] directly, this associated function handles
    /// emulator construction and wire remapping in a single step.
    pub fn predict<'src, 'dst, D, Ro>(
        routine: &Ro,
        input: &Bound<'src, D, Ro::Input>,
    ) -> Result<Prediction<Bound<'dst, Self, Ro::Output>, DriverValue<Self, Ro::Aux<'dst>>>>
    where
        D: Driver<'src, F = F, MaybeKind = M>,
        Ro: Routine<F>,
    {
        let input = StripWires::remap(input)?;
        routine.predict(&mut Self::wireless(), &input)
    }
}

impl<F: Field> Emulator<Wireless<Empty, F>> {
    /// Creates a new [`Emulator`] driver in [`Wireless`] mode, usually for
    /// counting wires or other static analysis on the circuit structure.
    pub fn counter() -> Self {
        Self::wireless()
    }
}

impl<F: Field> Emulator<Wireless<Always<()>, F>> {
    /// Creates a new [`Emulator`] driver in [`Wireless`] mode, specifically for
    /// executing with a known witness.
    pub fn execute() -> Self {
        Self::wireless()
    }

    /// Helper utility for executing a closure with a freshly created wireless
    /// [`Emulator`] when a witness is expected to exist.
    pub fn emulate_wireless<R, W: Send>(
        witness: W,
        f: impl FnOnce(&mut Self, Always<W>) -> Result<R>,
    ) -> Result<R> {
        let mut dr = Self::execute();
        dr.with(witness, f)
    }
}

impl<M: Mode<F = F>, F: Field> Emulator<M> {
    /// Helper utility for executing a closure with this [`Emulator`].
    fn with<R, W: Send>(
        &mut self,
        witness: W,
        f: impl FnOnce(&mut Self, Perhaps<M::MaybeKind, W>) -> Result<R>,
    ) -> Result<R> {
        f(self, M::MaybeKind::maybe_just(|| witness))
    }
}

impl<M: Mode> DriverTypes for Emulator<M> {
    type ImplField = M::F;
    type ImplWire = M::Wire;
    type MaybeKind = M::MaybeKind;
    type LCadd = M::LCadd;
    type LCenforce = M::LCenforce;
}

impl<'dr, M: MaybeKind, F: Field> Driver<'dr> for Emulator<Wireless<M, F>> {
    type F = F;
    type Wire = ();
    const ONE: Self::Wire = ();

    fn constant(&mut self, _: Coeff<Self::F>) -> Self::Wire {}

    fn mul(
        &mut self,
        _: impl Fn() -> Result<(Coeff<Self::F>, Coeff<Self::F>, Coeff<Self::F>)>,
    ) -> Result<(Self::Wire, Self::Wire, Self::Wire)> {
        Ok(((), (), ()))
    }

    fn add(&mut self, _: impl Fn(Self::LCadd) -> Self::LCadd) -> Self::Wire {}

    fn enforce_zero(&mut self, _: impl Fn(Self::LCenforce) -> Self::LCenforce) -> Result<()> {
        Ok(())
    }

    fn routine<R: Routine<Self::F> + 'dr>(
        &mut self,
        routine: R,
        input: Bound<'dr, Self, R::Input>,
    ) -> Result<Bound<'dr, Self, R::Output>> {
        short_circuit_routine(self, routine, input)
    }
}

impl<'dr, F: Field> Driver<'dr> for Emulator<Wired<F>> {
    type F = F;
    type Wire = F;
    const ONE: Self::Wire = F::ONE;

    fn constant(&mut self, coeff: Coeff<Self::F>) -> Self::Wire {
        coeff.value()
    }

    fn mul(
        &mut self,
        f: impl Fn() -> Result<(Coeff<Self::F>, Coeff<Self::F>, Coeff<Self::F>)>,
    ) -> Result<(Self::Wire, Self::Wire, Self::Wire)> {
        let (a, b, c) = f()?;

        // Despite wires existing, the emulator does not enforce multiplication
        // constraints.

        Ok((a.value(), b.value(), c.value()))
    }

    fn add(&mut self, lc: impl Fn(Self::LCadd) -> Self::LCadd) -> Self::Wire {
        let lc = lc(DirectSum::default());
        lc.value
    }

    fn enforce_zero(&mut self, _: impl Fn(Self::LCenforce) -> Self::LCenforce) -> Result<()> {
        // Despite wires existing, the emulator does not enforce linear
        // constraints.

        Ok(())
    }

    fn routine<R: Routine<Self::F> + 'dr>(
        &mut self,
        routine: R,
        input: Bound<'dr, Self, R::Input>,
    ) -> Result<Bound<'dr, Self, R::Output>> {
        short_circuit_routine(self, routine, input)
    }
}

/// The [`Emulator`] will short-circuit execution if the [`Routine`] can predict
/// its output, as the [`Emulator`] is not involved in enforcing any
/// constraints.
fn short_circuit_routine<'dr, D: Driver<'dr>, R: Routine<D::F> + 'dr>(
    dr: &mut D,
    routine: R,
    input: Bound<'dr, D, R::Input>,
) -> Result<Bound<'dr, D, R::Output>> {
    match routine.predict(dr, &input)? {
        Prediction::Known(output, _) => Ok(output),
        Prediction::Unknown(aux) => routine.execute(dr, input, aux),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Result;
    use crate::convert::WireMap;
    use crate::drivers::{Coeff, Driver};
    use crate::maybe::Always;
    use ff::Field;
    use ragu_pasta::Fp;

    type Emu = Emulator<Wireless<Always<()>, F>>;

    type F = Fp;

    // Manual Gadget impl because the derive macro cannot resolve `ragu_core` from within the crate.
    struct TwoWires<'dr, D: Driver<'dr>> {
        a: D::Wire,
        b: D::Wire,
        _marker: core::marker::PhantomData<&'dr ()>,
    }

    impl<'dr, D: Driver<'dr>> Clone for TwoWires<'dr, D> {
        fn clone(&self) -> Self {
            TwoWires {
                a: self.a.clone(),
                b: self.b.clone(),
                _marker: core::marker::PhantomData,
            }
        }
    }

    struct TwoWiresKind;

    /// # Safety
    /// `D::Wire: Send` implies `TwoWires<'dr, D>: Send` since the struct
    /// only contains wires and `PhantomData`.
    unsafe impl<FieldType: Field> crate::gadgets::GadgetKind<FieldType> for TwoWiresKind {
        type Rebind<'dr, D: Driver<'dr, F = FieldType>> = TwoWires<'dr, D>;

        fn map_gadget<
            'src,
            'dst,
            WM: WireMap<FieldType, Src: Driver<'src, F = FieldType>, Dst: Driver<'dst, F = FieldType>>,
        >(
            this: &Bound<'src, WM::Src, Self>,
            wm: &mut WM,
        ) -> Result<Bound<'dst, WM::Dst, Self>> {
            Ok(TwoWires {
                a: wm.convert_wire(&this.a)?,
                b: wm.convert_wire(&this.b)?,
                _marker: core::marker::PhantomData,
            })
        }

        fn enforce_equal_gadget<
            'dr,
            D1: Driver<'dr, F = FieldType>,
            D2: Driver<'dr, F = FieldType, Wire = <D1 as Driver<'dr>>::Wire>,
        >(
            dr: &mut D1,
            a: &Bound<'dr, D2, Self>,
            b: &Bound<'dr, D2, Self>,
        ) -> Result<()> {
            dr.enforce_equal(&a.a, &b.a)?;
            dr.enforce_equal(&a.b, &b.b)?;
            Ok(())
        }
    }

    impl<'dr, D: Driver<'dr>> crate::gadgets::Gadget<'dr, D> for TwoWires<'dr, D> {
        type Kind = TwoWiresKind;
    }

    // Alloc returns wires holding the assigned field values.
    #[test]
    fn wired_alloc_assigns_values() -> Result<()> {
        let mut dr = Emulator::<Wired<F>>::extractor();

        let w_one = dr.alloc(|| Ok(Coeff::One))?;
        let w_zero = dr.alloc(|| Ok(Coeff::Zero))?;
        let w_arb = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(42))))?;

        assert_eq!(w_one, F::ONE);
        assert_eq!(w_zero, F::ZERO);
        assert_eq!(w_arb, F::from(42));
        Ok(())
    }

    // Constant wires hold the expected field element for each Coeff variant.
    #[test]
    fn wired_constant_returns_correct_wire() -> Result<()> {
        let mut dr = Emulator::<Wired<F>>::extractor();

        let c_one = dr.constant(Coeff::One);
        let c_zero = dr.constant(Coeff::Zero);
        let c_neg = dr.constant(Coeff::NegativeOne);
        let c_arb = dr.constant(Coeff::Arbitrary(F::from(7)));

        assert_eq!(c_one, F::ONE);
        assert_eq!(c_zero, F::ZERO);
        assert_eq!(c_neg, -F::ONE);
        assert_eq!(c_arb, F::from(7));
        Ok(())
    }

    // The emulator accepts a*b != c without error since it does not enforce constraints.
    #[test]
    fn wired_mul_does_not_enforce_constraints() -> Result<()> {
        let mut dr = Emulator::<Wired<F>>::extractor();

        let (a, b, c) = dr.mul(|| {
            Ok((
                Coeff::Arbitrary(F::from(3)),
                Coeff::Arbitrary(F::from(5)),
                Coeff::Arbitrary(F::from(99)),
            ))
        })?;

        assert_eq!(a, F::from(3));
        assert_eq!(b, F::from(5));
        assert_eq!(c, F::from(99)); // emulator does not enforce a*b==c
        Ok(())
    }

    // Linear combination via add produces the correct accumulated value.
    #[test]
    fn wired_add_computes_linear_combination() -> Result<()> {
        let mut dr = Emulator::<Wired<F>>::extractor();

        let w1 = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(10))))?;
        let w2 = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(20))))?;

        // 1*w1 + 3*w2 = 10 + 60 = 70
        let sum = dr.add(|lc| lc.add(&w1).add_term(&w2, Coeff::Arbitrary(F::from(3))));

        assert_eq!(sum, F::from(70));
        Ok(())
    }

    // enforce_zero succeeds even for non-zero expressions since the emulator skips constraint checks.
    #[test]
    fn wired_enforce_zero_is_noop() -> Result<()> {
        let mut dr = Emulator::<Wired<F>>::extractor();

        let w = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(42))))?;

        let result = dr.enforce_zero(|lc| lc.add(&w));
        assert!(result.is_ok());
        Ok(())
    }

    // enforce_equal succeeds for unequal wires since the emulator skips constraint checks.
    #[test]
    fn wired_enforce_equal_is_noop() -> Result<()> {
        let mut dr = Emulator::<Wired<F>>::extractor();

        let w1 = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(1))))?;
        let w2 = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(999))))?;

        let result = dr.enforce_equal(&w1, &w2);
        assert!(result.is_ok());
        Ok(())
    }

    // emulate_wired runs circuit code and extracts the resulting wire field values.
    #[test]
    fn wired_emulate_wired_extracts_wires() -> Result<()> {
        let wires = Emulator::<Wired<F>>::emulate_wired((), |dr, _witness| {
            let a = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(5))))?;
            let b = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(10))))?;
            let sum = dr.add(|lc| lc.add(&a).add(&b));

            let gadget = TwoWires {
                a,
                b: sum,
                _marker: core::marker::PhantomData,
            };
            let extracted = dr.wires(&gadget)?;
            Ok(extracted)
        })?;

        assert_eq!(wires.len(), 2);
        assert_eq!(wires[0], F::from(5));
        assert_eq!(wires[1], F::from(15));
        Ok(())
    }

    // In wireless mode, all driver operations return () and witness closures are never called.
    #[test]
    fn wireless_execute_all_ops_return_unit() -> Result<()> {
        use core::cell::Cell;

        let mut dr = Emulator::<Wireless<Always<()>, F>>::execute();
        let called = Cell::new(0);

        let () = dr.alloc(|| {
            called.set(called.get() + 1);
            Ok(Coeff::Arbitrary(F::from(42)))
        })?;

        let () = dr.constant(Coeff::One);

        let ((), (), ()) = dr.mul(|| {
            called.set(called.get() + 1);
            Ok((
                Coeff::Arbitrary(F::from(3)),
                Coeff::Arbitrary(F::from(5)),
                Coeff::Arbitrary(F::from(15)),
            ))
        })?;

        let () = dr.add(|lc| {
            called.set(called.get() + 1);
            lc
        });

        let r = dr.enforce_zero(|lc| {
            called.set(called.get() + 1);
            lc
        });
        assert!(r.is_ok());
        assert_eq!(called.get(), 0);
        Ok(())
    }

    // Counter mode runs without witnesses, enabling static constraint counting.
    #[test]
    fn wireless_counter_static_analysis() -> Result<()> {
        let mut dr = Emulator::<Wireless<crate::maybe::Empty, F>>::counter();

        let () = dr.alloc(|| Ok(Coeff::One))?;

        let ((), (), ()) = dr.mul(|| Ok((Coeff::One, Coeff::One, Coeff::One)))?;

        let () = dr.add(|lc| lc);
        Ok(())
    }

    // ── DirectSum gain factor ───────────────────────────────────────

    #[test]
    fn direct_sum_gain_factor() {
        let acc = DirectSum::<F>::default()
            .add(&F::from(5))
            .gain(Coeff::Arbitrary(F::from(2)))
            .add(&F::from(3))
            .gain(Coeff::NegativeOne)
            .add(&F::from(4));

        assert_eq!(acc.value, F::from(3));
    }

    // A single-wire gadget for testing minimal wire counts.
    struct OneWire<'dr, D: Driver<'dr>> {
        w: D::Wire,
        _marker: core::marker::PhantomData<&'dr ()>,
    }

    impl<'dr, D: Driver<'dr>> Clone for OneWire<'dr, D> {
        fn clone(&self) -> Self {
            OneWire {
                w: self.w.clone(),
                _marker: core::marker::PhantomData,
            }
        }
    }

    struct OneWireKind;

    unsafe impl<FieldType: Field> GadgetKind<FieldType> for OneWireKind {
        type Rebind<'dr, D: Driver<'dr, F = FieldType>> = OneWire<'dr, D>;

        fn map_gadget<'dr, 'dr2, WM: WireMap<FieldType>>(
            this: &Bound<'dr, WM::Src, Self>,
            ndr: &mut WM,
        ) -> Result<Bound<'dr2, WM::Dst, Self>>
        where
            WM::Src: Driver<'dr, F = FieldType>,
            WM::Dst: Driver<'dr2, F = FieldType>,
        {
            Ok(OneWire {
                w: ndr.convert_wire(&this.w)?,
                _marker: core::marker::PhantomData,
            })
        }

        fn enforce_equal_gadget<
            'dr,
            D1: Driver<'dr, F = FieldType>,
            D2: Driver<'dr, F = FieldType, Wire = <D1 as Driver<'dr>>::Wire>,
        >(
            dr: &mut D1,
            a: &Bound<'dr, D2, Self>,
            b: &Bound<'dr, D2, Self>,
        ) -> Result<()> {
            dr.enforce_equal(&a.w, &b.w)?;
            Ok(())
        }
    }

    impl<'dr, D: Driver<'dr>> Gadget<'dr, D> for OneWire<'dr, D> {
        type Kind = OneWireKind;
    }

    // A stateful WireMap produces different wires each time the same gadget is mapped, confirming non-idempotent &mut self semantics.
    #[test]
    fn stateful_wiremap_produces_different_results_on_repeated_use() -> Result<()> {
        struct IncrementingMap {
            counter: u64,
        }

        impl WireMap<F> for IncrementingMap {
            type Src = Emulator<Wired<F>>;
            type Dst = Emulator<Wired<F>>;

            fn convert_wire(&mut self, _wire: &F) -> Result<F> {
                self.counter += 1;
                Ok(F::from(self.counter))
            }
        }

        let mut dr = Emulator::<Wired<F>>::extractor();
        let a = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(100))))?;
        let b = dr.alloc(|| Ok(Coeff::Arbitrary(F::from(200))))?;
        let gadget = TwoWires {
            a,
            b,
            _marker: core::marker::PhantomData,
        };

        let mut map = IncrementingMap { counter: 0 };

        let mapped1: TwoWires<'_, Emulator<Wired<F>>> = gadget.map(&mut map)?;
        assert_eq!(mapped1.a, F::from(1));
        assert_eq!(mapped1.b, F::from(2));

        let mapped2: TwoWires<'_, Emulator<Wired<F>>> = gadget.map(&mut map)?;
        assert_eq!(mapped2.a, F::from(3));
        assert_eq!(mapped2.b, F::from(4));

        assert_eq!(map.counter, 4);
        Ok(())
    }

    // A mid-gadget convert_wire failure leaves the WireMap in a dirty state that persists into subsequent mapping attempts.
    #[test]
    fn wiremap_partial_failure_leaves_dirty_state() -> Result<()> {
        struct FailOnEven {
            call_count: usize,
        }

        impl WireMap<F> for FailOnEven {
            type Src = Emu;
            type Dst = core::marker::PhantomData<F>;

            fn convert_wire(&mut self, _: &()) -> Result<()> {
                self.call_count += 1;
                if self.call_count.is_multiple_of(2) {
                    Err(crate::Error::InvalidWitness("even call".into()))
                } else {
                    Ok(())
                }
            }
        }

        let gadget: TwoWires<'_, Emu> = TwoWires {
            a: (),
            b: (),
            _marker: core::marker::PhantomData,
        };

        let mut map = FailOnEven { call_count: 0 };

        let result = gadget.map(&mut map);
        assert!(result.is_err());
        assert_eq!(map.call_count, 2);

        let one_wire: OneWire<'_, Emu> = OneWire {
            w: (),
            _marker: core::marker::PhantomData,
        };
        let result2 = one_wire.map(&mut map);
        assert!(result2.is_ok());
        assert_eq!(map.call_count, 3);

        let result3 = gadget.map(&mut map);
        assert!(result3.is_err());
        assert_eq!(map.call_count, 4);

        Ok(())
    }
}
