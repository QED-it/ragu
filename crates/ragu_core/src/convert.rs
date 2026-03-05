//! Conversions that translate gadgets from one [`Driver`] context to another.
//!
//! Gadgets are polymorphic over drivers, and translating a gadget from one
//! driver context to another while preserving its structure and semantics is a
//! fundamental operation. Any code that operates across multiple driver
//! contexts will need this: [routines](crate::routines) translate their inputs
//! onto [`Wireless`] [`Emulator`]s for prediction, wire-counting passes discard
//! wire values entirely, and driver implementations may need to inject or
//! rewrite wires during circuit analysis.
//!
//! [`WireMap`] provides a uniform mechanism for these conversions: an
//! implementor fixes a source and destination driver via associated types, then
//! converts wires one at a time. Because [`WireMap`] is a separate trait rather
//! than an associated type on [`GadgetKind`](crate::gadgets::GadgetKind), the
//! same gadget kind can be remapped by many different conversion strategies
//! without adding type parameters to
//! [`GadgetKind`](crate::gadgets::GadgetKind).
//!
//! ### Public API
//!
//! - [`WireMap`], the core conversion trait.
//! - [`CloneWires`], a [`WireMap`] that clones wires unchanged.
//! - [`StripWires`], a [`WireMap`] that discards wire values for use with
//!   wireless emulators.
//!
//! [`Routine::predict`]: crate::routines::Routine::predict

use core::marker::PhantomData;
use ff::Field;

use crate::{
    Result,
    drivers::emulator::{Emulator, Wireless},
    drivers::{Driver, DriverTypes},
    gadgets::{Bound, Gadget},
};

/// Conversion context that maps wires from one driver to another.
///
/// Each implementor fixes a specific source and destination driver via
/// associated types. When the same conversion logic applies to multiple
/// source/destination pairs, use a wrapper struct parameterized by those
/// types. See [`WirelessFrom`](crate::drivers::emulator::WirelessFrom)
/// for an example.
///
/// `Src` and `Dst` are bounded by [`DriverTypes`] here so that
/// `convert_wire` can name their wire types. The stronger [`Driver`] bound
/// is only required at call sites that actually traverse a gadget (for instance
/// [`Gadget::map`] and [`GadgetKind::map_gadget`](crate::gadgets::GadgetKind::map_gadget)),
/// where it appears as a where-clause on the method rather than on the
/// trait itself. This keeps `WireMap` implementors free of lifetime
/// parameters while still ensuring full `Driver` constraints are checked
/// wherever wires are converted through a gadget.
pub trait WireMap<F: Field> {
    /// The source driver whose wires are being converted.
    ///
    /// Must implement [`Driver<'dr>`](Driver) at every call site that
    /// passes this `WireMap` to [`Gadget::map`].
    type Src: DriverTypes<ImplField = F>;

    /// The destination driver whose wires are produced.
    ///
    /// Must implement [`Driver<'dr>`](Driver) at every call site that
    /// passes this `WireMap` to [`Gadget::map`].
    type Dst: DriverTypes<ImplField = F>;

    /// Converts a wire from the source driver to the destination driver.
    fn convert_wire(
        &mut self,
        wire: &<Self::Src as DriverTypes>::ImplWire,
    ) -> Result<<Self::Dst as DriverTypes>::ImplWire>;
}

/// A [`WireMap`] that passes wires through unchanged by cloning them.
///
/// The source and destination must share the same wire type, so conversion
/// calls [`.clone()`](Clone::clone) on each wire. This is useful when
/// rebinding a gadget to a new lifetime without changing its wire
/// representation.
pub struct CloneWires<Src: DriverTypes, Dst: DriverTypes>(PhantomData<(Src, Dst)>);

impl<Src: DriverTypes, Dst: DriverTypes> Default for CloneWires<Src, Dst> {
    fn default() -> Self {
        CloneWires(PhantomData)
    }
}

impl<F: Field, Src, Dst> CloneWires<Src, Dst>
where
    Src: DriverTypes<ImplField = F>,
    Dst: DriverTypes<ImplField = F, ImplWire = Src::ImplWire>,
{
    /// Maps a gadget to a destination driver by cloning its wires.
    ///
    /// `Src` is inferred from the gadget; `Dst` can be inferred from the
    /// return context or spelled out explicitly:
    ///
    /// ```ignore
    /// // Inferred from context:
    /// let output: Bound<'_, DstDriver, _> = CloneWires::convert(&gadget)?;
    /// // Explicit:
    /// let output = CloneWires::<_, DstDriver>::convert(&gadget)?;
    /// ```
    pub fn convert<'src, 'dst, G: Gadget<'src, Src>>(
        gadget: &G,
    ) -> Result<Bound<'dst, Dst, G::Kind>>
    where
        Src: Driver<'src, F = F>,
        Dst: Driver<'dst, F = F, Wire = Src::Wire>,
    {
        gadget.map(&mut Self::default())
    }
}

impl<F: Field, Src, Dst> WireMap<F> for CloneWires<Src, Dst>
where
    Src: DriverTypes<ImplField = F>,
    Dst: DriverTypes<ImplField = F, ImplWire = Src::ImplWire>,
{
    type Src = Src;
    type Dst = Dst;

    fn convert_wire(
        &mut self,
        wire: &<Src as DriverTypes>::ImplWire,
    ) -> Result<<Dst as DriverTypes>::ImplWire> {
        Ok(wire.clone())
    }
}

/// A [`WireMap`] that maps any driver's wires to `()`, discarding wire
/// values for use with `Emulator<Wireless<D::MaybeKind, D::ImplField>>`.
///
/// This conversion is used to pass a gadget from a concrete driver into
/// [`Routine::predict`], which operates on a [`Wireless`] emulator. The
/// wrapper struct is parameterized by the source driver so that each source
/// type gets its own blanket [`WireMap`] impl.
///
/// [`Routine::predict`]: crate::routines::Routine::predict
pub struct StripWires<D: DriverTypes>(PhantomData<D>);

impl<D: DriverTypes> Default for StripWires<D> {
    fn default() -> Self {
        StripWires(PhantomData)
    }
}

impl<F: Field, D> StripWires<D>
where
    D: DriverTypes<ImplField = F>,
{
    /// Maps a gadget to a wireless emulator by discarding its wires.
    pub fn strip<'src, 'dst, G: Gadget<'src, D>>(
        gadget: &G,
    ) -> Result<Bound<'dst, Emulator<Wireless<D::MaybeKind, F>>, G::Kind>>
    where
        D: Driver<'src, F = F>,
    {
        gadget.map(&mut Self::default())
    }
}

impl<F: Field, D: DriverTypes<ImplField = F>> WireMap<F> for StripWires<D> {
    type Src = D;
    type Dst = Emulator<Wireless<D::MaybeKind, F>>;

    fn convert_wire(&mut self, _: &D::ImplWire) -> Result<()> {
        Ok(())
    }
}
