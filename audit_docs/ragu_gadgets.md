## Background for Understanding `ragu_gadgets`

### What the crate does

`ragu_gadgets` is currently an **empty stub**. The `lib.rs` contains only a doc comment header and no code. The `Cargo.toml` has no dependencies and marks the crate as WIP.

The intended purpose (based on its position in the dependency tree between `ragu_primitives` and `ragu_circuits`) is to house higher-level reusable gadget compositions — circuit building blocks that combine the primitive gadgets (Boolean, Element, Point, Endoscalar) into more complex structures.

Currently, all gadgets used by the protocol are defined directly in `ragu_primitives` (low-level) or inline within `ragu_circuits` and `ragu_pcd` (protocol-specific). This crate would eventually provide a middle layer of general-purpose gadgets.

### Audit note

Nothing to review. Skip this crate.
