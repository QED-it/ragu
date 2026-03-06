# Conversion

Drivers often need to substitute wires in a gadget, inspect its internal layout,
or move it into a different [driver](../drivers/index.md) context. Conversion
supports all of these through a visitor that walks a gadget's wire tree, passing
each wire through a pluggable (possibly stateful) transformation.
[Fungibility](index.md#fungibility) guarantees that the result is a valid gadget
of the same kind, with structure and semantics preserved.

## [`WireMap`][wiremap-trait]

The [`WireMap`][wiremap-trait] trait provides a uniform mechanism for these
conversions. An implementor fixes a source and destination driver via associated
types and defines a strategy for transforming wires between them one at a time:

```rust,ignore
pub trait WireMap<F: Field> {
    type Src: DriverTypes<ImplField = F>;
    type Dst: DriverTypes<ImplField = F>;

    fn convert_wire(
        &mut self,
        wire: &<Self::Src as DriverTypes>::ImplWire,
    ) -> Result<<Self::Dst as DriverTypes>::ImplWire>;
}
```

[`GadgetKind::map_gadget`][map-gadget-method] performs the actual traversal,
walking the gadget's fields and dispatching each one according to its kind.
`Wire` fields go through [`convert_wire`][convert-wire], `DriverValue` fields
are reconstructed via [`Maybe::just`][maybe-just] (preserving or discarding
witness data according to the destination driver's
[`MaybeKind`][maybekind-trait]), and nested gadget fields recurse. The
[`Gadget::map`][gadget-map] method is a convenience proxy for
[`map_gadget`][map-gadget-method].

```admonish tip
[`WireMap`][wiremap-trait] also provides a [`remap`][remap-method] shorthand
for wire maps that implement [`Default`]: it constructs a fresh instance and
maps the gadget in one call. All built-in wire maps support this.
```

## [`CloneWires`][clonewires-type]

[`CloneWires`][clonewires-type] is a pass-through conversion for drivers that
share the same wire type. Each wire is cloned unchanged, moving the gadget into
the destination driver's context:

```rust,ignore
let output: Bound<'dst, DstDriver, _> = CloneWires::remap(&gadget)?;
```

## [`StripWires`][stripwires-type]

[`StripWires`][stripwires-type] maps any driver's wires to `()`, producing a
gadget bound to a wireless [`Emulator`] with the same
[`MaybeKind`][maybekind-trait] as the source driver. This preserves witness
availability while stripping wire structure.

The primary use case is [routine prediction](../routines.md): routines receive
their input on a wireless emulator so they can compute predicted outputs
without a real synthesis context. [`StripWires`][stripwires-type] handles the
conversion from the caller's driver to that emulator automatically.

[wiremap-trait]: ragu_core::convert::WireMap
[convert-wire]: ragu_core::convert::WireMap::convert_wire
[clonewires-type]: ragu_core::convert::CloneWires
[stripwires-type]: ragu_core::convert::StripWires
[remap-method]: ragu_core::convert::WireMap::remap
[gadgetkind-trait]: ragu_core::gadgets::GadgetKind
[map-gadget-method]: ragu_core::gadgets::GadgetKind::map_gadget
[gadget-map]: ragu_core::gadgets::Gadget::map
[maybe-just]: ragu_core::maybe::Maybe::just
[maybekind-trait]: ragu_core::maybe::MaybeKind
[`Default`]: core::default::Default
[`Emulator`]: ragu_core::drivers::emulator::Emulator
