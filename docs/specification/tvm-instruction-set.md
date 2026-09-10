# ONX Specification — TVM Instruction Set (Basic Workchain)

**Status:** Draft
**Scope:** Concrete bytecode opcode encoding, stack value model, and per-instruction gas pricing for the basic-workchain virtual machine whose execution contract `docs/specification/execution.md` already defines. This is the "future TVM Instruction Set artifact" that document's §3.1 and §3.4 explicitly deferred.

---

## 1. Reference

- `docs/specification/execution.md` §3.1–§3.4: the execution contract (`execute(code, data, message, context) -> ...`), the six required semantic categories, the closed five-member `ExceptionKind` set, and the explicit deferral of concrete opcodes and gas pricing to this document.
- `docs/specification/execution.md` §3.5, ADR-0007: the reserved Merkle-proof "pruned branch" special-cell semantics and `AbsentNode`, which this instruction set's cell-access opcodes must respect.
- `docs/specification/state-model.md` §3.3–§4.2: the `Cell`/`BagOfCells` representation this instruction set's `code` and `data` operate on, including `MAX_CELL_DATA_BYTES = 128`, `MAX_CELL_REFS = 4`, and the `is_special` flag.
- `docs/specification/protocol-primitives.md`: canonical fixed-width integer encoding (`Uint8`–`Uint256`, `Int8`–`Int256`) and domain-separated SHA-256/Ed25519 primitives this instruction set's arithmetic and cryptographic opcodes reuse rather than redefine.
- `docs/specification/transactions.md`: `MAX_TENTATIVE_GAS = 10,000`, the External Inbound tentative-execution gas cap this document's cryptographic opcode pricing (§3.6) is checked against for plausibility.
- `WHITEPAPER.md` §2.1.20: TVM's feature list (stack machine, cell-based value model, algebraic data types, fixed-width arithmetic in unsigned/signed/modulo flavors, bit/byte strings, ECC and hash primitives, closures) — as `execution.md` §3.1 already notes, this is a feature list, not an opcode table; no such table exists in the available reference material, so this document is an ONX-original design within the constraints that feature list and `execution.md`'s contract establish, not a transcription of any existing implementation (`INSTRUCTIONS.md` §1, §6).
- `WHITEPAPER.md` §5.1.9, `docs/specification/payment-channels.md`: the light-client Merkle-proof-as-ordinary-value use case that motivates keeping cell-reference-passing and hashing available on pruned branches even though content access is not (§3.5).
- `docs/specification/architecture.md`: open question **ONX-ARCH-006** (VM rules per workchain), partially resolved by `execution.md`; this document resolves the "concrete instruction set" portion for the basic workchain specifically. Which VM(s) other workchains use remains open.

---

## 2. Requirement

`execution.md` §3.1 requires a separate artifact defining, before any ONX VM can be implemented in code:
1. A concrete bytecode instruction encoding covering every semantic category `execution.md` §3.3 requires as a minimum (fixed-width arithmetic in three flavors, overflow checking, explicit-width conversion, bit/byte-string operations, cell/value access, and cryptographic primitive access).
2. A stack value model concrete enough to implement, since `execution.md` deliberately left the VM's internal state shape unspecified beyond the black-box `execute(...)` contract.
3. A deterministic, per-instruction gas cost table, since `execution.md` §3.4 requires such a function to exist and be identical across nodes but explicitly deferred its concrete prices here.
4. A resolution of every bytecode-level fault condition (invalid opcode, stack underflow, out-of-range operand) into `execution.md` §3.4's existing closed five-member `ExceptionKind` set, since that set may not be silently extended.

---

## 3. ONX Interpretation

### 3.1 Scope: a minimum sufficient instruction set, not an exhaustive one

This document defines the smallest instruction set that satisfies every requirement in `execution.md` §3.3 plus the control flow needed to make execution possible at all (§3.5 below). It is **not** an attempt to reproduce any existing VM's full opcode table — per `INSTRUCTIONS.md` §1 and §6, existing implementations are secondary research material, not an authority, and per §22's performance-last ordering, code-density and instruction-count optimizations (short-immediate variants, fused compare-and-branch opcodes, and so on) are explicitly deferred to a future revision once real contracts expose which patterns are actually common. Adding opcodes later is backward compatible (new opcode bytes from the reserved ranges in §4.1); this document only commits to the opcodes it defines, not to being final.

### 3.2 Stack value model

The VM is a stack machine (`WHITEPAPER.md` §2.1.20) with **no first-class continuations**: control flow (§3.5) uses a simple internal call-stack of return addresses, not the closures `WHITEPAPER.md`'s feature list mentions. `execution.md` §3.3 does not require closures, and per `WHITEPAPER.md` §2.8.16–§2.8.17's own warning about how costly it is to retrofit foundational VM semantics, ONX defers first-class continuations rather than half-committing to them now; a future revision may add them as a wholly new value kind without breaking this document's opcodes.

The operand stack holds values of exactly five kinds:

| Kind | Meaning |
| --- | --- |
| `Integer` | An arbitrary-precision two's-complement value in the closed range `[-2^256, 2^256 - 1]` — a superset of every width this instruction set's arithmetic operates at. Each arithmetic/conversion opcode is parameterized with the *declared* width and signedness it checks against (§3.3); the stack representation itself is untyped by width. |
| `Bytes` | A variable-length byte string (§3.4), independent of any `Cell`. |
| `Cell` | A reference to a `state-model.md` `Cell`, opaque until read via `CTOS` (§3.5.3). |
| `Slice` | A read cursor `(cell, bit_offset, ref_offset)` over one `Cell`'s data bits and child references, produced by `CTOS` and advanced by `LD*` opcodes. |
| `Builder` | A write accumulator `(bits_so_far, refs_so_far)` for constructing a new `Cell`, produced by `NEWC` and finalized by `ENDC`. |

None of these five kinds, nor the operand stack itself, is ever serialized: the only values that cross a canonical wire boundary are `code`, `data` (both `Cell`s, per `state-model.md`), and `out_messages` (`transactions.md` `Message`s), exactly as `execution.md` §3.2 already established. §4 below therefore defines only the **instruction encoding within a `code` Cell's data bits**, not a serialization for stack values.

`execution.md` §3.3 rule 5 requires "dereferencing child-cell references as typed algebraic values." This instruction set does not add a separate tagged-value or tuple mechanism for that: an algebraic type is, at the VM level, just a `Slice` whose leading bits a contract reads as a discriminant tag via `LDU` (§3.5.3) and branches on via `IFJMPREF`/`IFNOTJMPREF` (§3.5.4) — ordinary use of the primitives below, not new VM machinery. This mirrors how `data-structures.md` and `transactions.md` themselves define algebraic layouts (a tag byte followed by type-dependent fields) without any VM-level tag mechanism beyond raw bit reads.

### 3.3 Arithmetic: width and flavor as instruction operands

Every arithmetic and conversion opcode (§4.2) carries a `width : uint16` (`1 ≤ width ≤ 256`) and, where signedness matters, a `flavor : uint8` operand (`0` = unsigned, `1` = signed, `2` = modulo), rather than one opcode per width — the reverse of a typical fixed-width-per-opcode ISA, chosen because `execution.md` §3.3 requires "at least 64-, 128-, and 256-bit widths" without capping the set of required widths, and a distinct opcode per width would multiply the opcode space for no semantic benefit.

- **Unsigned/signed flavors** (`0`/`1`) raise `IntegerOverflow` (`execution.md` §3.4) when the true mathematical result does not fit in `width` bits under that signedness — per `execution.md` §3.3 rule 2, this is the default, not opt-in.
- **Modulo flavor** (`2`) never raises `IntegerOverflow`: the result is reduced modulo `2^width` and stored as its unsigned bit pattern (per `execution.md` §3.3 rule 1's "no automatic overflow checks in the modulo flavor only"). A contract wanting wrapped *signed* semantics reinterprets the same bit pattern via a signed-flavor `CONV` (§4.2), since two's-complement wraparound is bit-identical regardless of the signedness label attached afterward.
- **Division/modulo by zero** (`DIVMOD`, §4.2) raises `IntegerOverflow`: no result exists that could fit any declared width, and `execution.md`'s closed exception set has no dedicated "arithmetic fault" kind, so this document maps it to the closest existing one rather than extending the set.

### 3.4 Bit-strings and byte-strings

`execution.md` §3.3 rule 4 requires bit-string and byte-string operations "consistent with `protocol-primitives.md`'s canonical bitstring/byte-string encodings." This instruction set represents such values as the `Bytes` stack kind (§3.2): a flat byte sequence, manipulated via `BYTELEN`/`CONCAT`/`SUBBYTES`/`BYTEEQ` (§4.3). A `Bytes` value becomes bit-exact `Cell` content only once written into a `Builder` via `STBYTES` (§4.4) — consistent with `protocol-primitives.md` treating "canonical bytes" and "canonical fixed-width integers" as the two building blocks every larger structure serializes from.

### 3.5 Cell access and control flow

#### 3.5.1 Code layout

A contract's `code` (`execution.md` §3.2) is a `Cell` whose up to `MAX_CELL_DATA_BYTES = 128` data bytes (`state-model.md` §4.2) are interpreted as a flat instruction stream starting at bit offset 0, and whose up to `MAX_CELL_REFS = 4` child-cell references are addressable as **code continuations** by `JMPREF`/`CALLREF`/`IFJMPREF`/`IFNOTJMPREF`/`IFCALLREF`/`IFNOTCALLREF` (§4.5) — a program larger than one `Cell`'s data capacity is split across a tree of code `Cell`s exactly the way any other over-128-byte value already must be, per `state-model.md`'s existing limits. This is a deliberate reuse of an existing constraint rather than a new one introduced for code specifically.

#### 3.5.2 Call stack, not general continuations

`CALLREF` (§4.5) pushes a return address `(code_cell, bit_offset)` onto an internal call stack (not an operand-stack value — it is not inspectable or duplicable by contract code); `RET` pops it. No explicit call-stack depth limit is imposed: because `CALLREF` costs gas per invocation (§4.5's table), the existing gas limit already bounds recursion depth, consistent with `execution.md` §3.4 requiring resource exhaustion to be enforced uniformly through gas rather than through a second, independent limit.

#### 3.5.3 Pruned branches and `AbsentNode`

Per `execution.md` §3.5, a `Cell` may be a pruned Merkle-proof branch (`is_special = true`, §3.2's `Cell.is_special_flag`). This instruction set draws the `AbsentNode` boundary at **content access**, not at reference-passing or hashing, matching `WHITEPAPER.md` §5.1.9's design intent that a Merkle proof's *shape and hash* remain usable even where its *content* is deliberately absent:

- `CTOS` (§4.4) raises `AbsentNode` if its operand `Cell` is a pruned special cell — this is the only opcode that raises it.
- `LDREF` (§4.4) never raises `AbsentNode`: it hands back the child `Cell` reference itself (pruned or not) without reading its content, so a contract can pass a pruned reference along (e.g. store it, or hash it) without being forced to dereference it.
- `HASHCELL` (§4.6) never raises `AbsentNode`: a pruned cell's committed hash is exactly the information a Merkle proof supplies, and hashing it requires no hidden content.
- `ISEXOTIC` (§4.4) never raises anything: it inspects only the cell descriptor's special-cell bit, which is present on every cell regardless of pruning.

#### 3.5.4 Bytecode-well-formedness faults

An unrecognized opcode byte, a `ref_index` operand pointing outside the current code `Cell`'s actual reference count, or an operand-stack depth insufficient for an opcode's declared arity are all **bytecode-well-formedness** violations — distinct from `execution.md` §3.4's runtime exceptions, which describe valid, well-formed code encountering a value it cannot process. Both are nonetheless required to map into the existing closed `ExceptionKind` set (`execution.md` §3.4: "must not raise any exception outside this list without an ONX specification amendment"); this document, as that amendment, maps all three to `MalformedCell`, reasoning that a `code` `Cell` whose instruction stream cannot be decoded, whose reference operand is out of range, or whose surrounding instruction sequence violates an opcode's stack-effect contract is itself a malformed `Cell` under an extension of `state-model.md` §5's structural rules to a `code` `Cell`'s decode-time and dispatch-time validity, not only its leaf data/reference-count limits.

### 3.6 Gas pricing and consistency with `MAX_TENTATIVE_GAS`

§4's per-instruction costs are chosen on a simple relative scale — cheap stack/comparison operations cost `1`, arithmetic costs `4`–`8`, cell access costs `10`, hashing costs `200`, and Ed25519 verification costs `4000` — reflecting relative real compute cost, not a claim of calibrated final pricing (`INSTRUCTIONS.md` §7: illustrative numbers are not binding without their own decision; this document *is* that decision for a first baseline, revisable by a future ADR once real execution profiling exists). As a plausibility check: `transactions.md`'s `MAX_TENTATIVE_GAS = 10,000` must be enough to tentatively execute a typical External Inbound message's signature check — one `CHKSIGNU` (`4000`) plus a handful of `LDU`/`LDREF` cell reads (`10` each) leaves ample headroom under `10,000`, so the two documents' numbers are mutually consistent rather than accidentally incompatible.

---

## 4. Serialization

### 4.1 Opcode byte ranges

```
0x00-0x0F: Stack manipulation
0x10-0x1F: Arithmetic
0x20-0x2F: Integer conversion
0x30-0x3F: Bit-string / byte-string
0x40-0x5F: Cell / value access
0x60-0x6F: Cryptographic primitives
0x70-0x7F: Control flow
0x80-0xFF: Reserved for future extension
```

Every instruction is `opcode : uint8` followed by zero or more immediate operand bytes fixed in number and size for that opcode (no variable-length operands except a literal's own declared length, as in `PUSHBYTES`). All multi-byte immediates are big-endian, matching `protocol-primitives.md`'s convention.

### 4.2 Stack manipulation (`0x00`–`0x0F`)

| Opcode | Mnemonic | Operands | Stack effect | Gas |
| --- | --- | --- | --- | --- |
| `0x00` | `NOP` | — | `() -> ()` | 1 |
| `0x01` | `DROP` | — | `(a) -> ()` | 1 |
| `0x02` | `DUP` | — | `(a) -> (a, a)` | 1 |
| `0x03` | `SWAP` | — | `(a, b) -> (b, a)` | 1 |
| `0x04` | `OVER` | — | `(a, b) -> (a, b, a)` | 1 |
| `0x05` | `ROT` | — | `(a, b, c) -> (b, c, a)` | 1 |
| `0x06` | `PICK` | `depth: uint8` | copies `stack[depth]` (0 = top) to top | 1 |
| `0x07` | `ROLL` | `depth: uint8` | moves `stack[depth]` to top | 1 |
| `0x08` | `PUSHINT` | `signed: uint8`, `value: [u8; 32]` | `() -> (Integer)` | 1 |
| `0x09` | `PUSHBYTES` | `len: uint16`, then `len` raw bytes | `() -> (Bytes)` | `1 + ceil(len / 32)` |
| `0x0A` | `NIP` | — | `(a, b) -> (b)` | 1 |
| `0x0B` | `TUCK` | — | `(a, b) -> (b, a, b)` | 1 |
| `0x0C` | `BLKSWAP` | `left: uint8`, `right: uint8` | swaps the two adjacent top blocks | 1 |

All thirteen raise `MalformedCell` if the instruction requires more stack items than are present, if `PICK`/`ROLL`'s `depth` is not a valid stack index, or if `BLKSWAP` names an empty or unavailable block (§3.5.4). The operand stack is capped at 1023 elements; an instruction that would exceed the cap raises `MalformedCell`.

### 4.3 Arithmetic, conversion, bit-strings (`0x10`–`0x3F`)

| Opcode | Mnemonic | Operands | Stack effect | Gas | Exceptions |
| --- | --- | --- | --- | --- | --- |
| `0x10` | `ADD` | `width: uint16`, `flavor: uint8` | `(a, b) -> (a + b)` | 4 | `IntegerOverflow` |
| `0x11` | `SUB` | `width`, `flavor` | `(a, b) -> (a - b)` | 4 | `IntegerOverflow` |
| `0x12` | `NEG` | `width`, `flavor` | `(a) -> (-a)` | 4 | `IntegerOverflow` |
| `0x13` | `MUL` | `width`, `flavor` | `(a, b) -> (a * b)` | 8 | `IntegerOverflow` |
| `0x14` | `DIVMOD` | `width`, `flavor` | `(a, b) -> (a div b, a mod b)`, floored | 8 | `IntegerOverflow` (incl. `b = 0`, §3.3) |
| `0x15` | `CMP` | `width`, `flavor` | `(a, b) -> (r)`, `r ∈ {-1, 0, 1}` as a signed 8-bit `Integer` | 4 | — |
| `0x16` | `ISZERO` | — | `(a) -> (bool)`, `bool` a 1-bit unsigned `Integer` | 4 | — |
| `0x17` | `DIV` | `width`, `flavor` | `(a, b) -> (a div b)` | 8 | `IntegerOverflow` (incl. `b = 0`) |
| `0x18` | `LSHIFT` | `width`, `flavor` | `(a, shift) -> (a << shift)` | 4 | `IntegerOverflow` |
| `0x19` | `RSHIFT` | `width`, `flavor` | `(a, shift) -> (a >> shift)` | 4 | `IntegerOverflow` |
| `0x20` | `CONV` | `width: uint16`, `signed: uint8` | `(a) -> (a')`, re-checked at `width` | 4 | `IntegerOverflow` |
| `0x30` | `BYTELEN` | — | `(Bytes) -> (Integer)`, unsigned 32-bit | 1 | — |
| `0x31` | `CONCAT` | — | `(Bytes, Bytes) -> (Bytes)` | `4 + ceil(total_len / 32)` | — |
| `0x32` | `SUBBYTES` | — | `(Bytes, offset: Integer, len: Integer) -> (Bytes)` | `4 + ceil(len / 32)` | `MalformedCell` if `offset + len` exceeds the input length |
| `0x33` | `BYTEEQ` | — | `(Bytes, Bytes) -> (bool)` | `1 + ceil(min(len1, len2) / 32)` | — |

`0x17`–`0x1F`, `0x21`–`0x2F`, and `0x34`–`0x3F` are reserved.

### 4.4 Cell / value access (`0x40`–`0x5F`)

| Opcode | Mnemonic | Operands | Stack effect | Gas | Exceptions |
| --- | --- | --- | --- | --- | --- |
| `0x40` | `NEWC` | — | `() -> (Builder)`, empty | 10 | — |
| `0x41` | `ENDC` | — | `(Builder) -> (Cell)` | 10 | — |
| `0x42` | `STBITS` | `width: uint16`, `signed: uint8` | `(Builder, Integer) -> (Builder)` | 10 | `IntegerOverflow` if the value doesn't fit `width`; `MalformedCell` if appending would exceed 128 bytes |
| `0x43` | `STREF` | — | `(Builder, Cell) -> (Builder)` | 10 | `MalformedCell` if the builder already has 4 references |
| `0x44` | `STBYTES` | — | `(Builder, Bytes) -> (Builder)` | `10 + ceil(len / 32)` | `MalformedCell` if appending would exceed 128 bytes |
| `0x45` | `CTOS` | — | `(Cell) -> (Slice)`, at `(0, 0)` | 10 | `AbsentNode` if `Cell` is a pruned special cell (§3.5.3) |
| `0x46` | `LDU` | `width: uint16` | `(Slice) -> (Slice, Integer)`, unsigned | 10 | `MalformedCell` if fewer than `width` bits remain |
| `0x47` | `LDI` | `width: uint16` | `(Slice) -> (Slice, Integer)`, signed | 10 | `MalformedCell` if fewer than `width` bits remain |
| `0x48` | `LDREF` | — | `(Slice) -> (Slice, Cell)` | 10 | `MalformedCell` if no references remain; never `AbsentNode` (§3.5.3) |
| `0x49` | `ISEXOTIC` | — | `(Cell) -> (bool)` | 10 | — |
| `0x4A` | `SEMPTY` | — | `(Slice) -> (bool)`, true iff both bits and refs are exhausted | 1 | — |
| `0x4B` | `SBITS` | — | `(Slice) -> (Integer)`, remaining bits, unsigned 16-bit | 1 | — |
| `0x4C` | `SREFS` | — | `(Slice) -> (Integer)`, remaining refs, unsigned 8-bit | 1 | — |

`0x4D`–`0x5F` are reserved.

### 4.5 Control flow (`0x70`–`0x7F`)

| Opcode | Mnemonic | Operands | Effect | Gas | Exceptions |
| --- | --- | --- | --- | --- | --- |
| `0x70` | `JMPREF` | `ref_index: uint8` | jump to child cell `ref_index`'s start | 4 | `MalformedCell` if out of range (§3.5.4) |
| `0x71` | `CALLREF` | `ref_index: uint8` | push return address, then jump as `JMPREF` | 4 | `MalformedCell` if out of range |
| `0x72` | `RET` | — | pop the call stack and resume there; terminate successfully if empty | 4 | — |
| `0x73` | `IFJMPREF` | `ref_index: uint8` | pop `Integer`; `JMPREF` if nonzero | 4 | `MalformedCell` if out of range and taken |
| `0x74` | `IFNOTJMPREF` | `ref_index: uint8` | pop `Integer`; `JMPREF` if zero | 4 | `MalformedCell` if out of range and taken |
| `0x75` | `IFCALLREF` | `ref_index: uint8` | pop `Integer`; `CALLREF` if nonzero | 4 | `MalformedCell` if out of range and taken |
| `0x76` | `IFNOTCALLREF` | `ref_index: uint8` | pop `Integer`; `CALLREF` if zero | 4 | `MalformedCell` if out of range and taken |
| `0x77` | `THROW` | `kind: uint8` (`0`=`IntegerOverflow`, `1`=`AbsentNode`, `2`=`MalformedCell`, `3`=`TypeMismatch`) | unconditionally raise `kind` | 4 | the named kind, always |
| `0x78` | `IFELSE` | `true_offset: int8`, `false_offset: int8` | pop condition and branch by byte offset | 4 | `MalformedCell` for an out-of-range target |
| `0x79` | `IFRET` | — | pop condition and return if nonzero | 4 | — |
| `0x7A` | `REPEAT` | `count: uint8`, `offset: int8` | re-enter preceding block when count is nonzero | 4 | `MalformedCell` for an out-of-range target |
| `0x7B` | `UNTIL` | `offset: int8` | pop condition and re-enter preceding block while zero | 4 | `MalformedCell` for an out-of-range target |

`0x7C`–`0x7F` are reserved. `THROW` cannot target `OutOfGas`: that kind is raised only by the VM's own gas metering (`execution.md` §3.4), never by contract-directed control flow.

### 4.6 Cryptographic primitives (`0x60`–`0x6F`)

| Opcode | Mnemonic | Operands | Stack effect | Gas | Exceptions |
| --- | --- | --- | --- | --- | --- |
| `0x60` | `HASHBYTES` | — | `(Bytes) -> (Integer)`, unsigned 256-bit SHA-256 (`protocol-primitives.md`) | 200 | — |
| `0x61` | `HASHCELL` | — | `(Cell) -> (Integer)`, unsigned 256-bit domain-separated Cell hash (`state-model.md`'s `ONX_CELL_HASH_V1`) | 200 | never `AbsentNode` (§3.5.3) |
| `0x62` | `CHKSIGNU` | — | `(pubkey: Bytes, signature: Bytes, hash: Integer) -> (bool)`, Ed25519 verify (`protocol-primitives.md`) | 4000 | `TypeMismatch` if `pubkey` is not exactly 32 bytes or `signature` is not exactly 64 bytes |

`0x63`–`0x6F` are reserved.

---

## 5. Malformed-input behavior

A conforming VM implementation MUST raise the indicated `ExceptionKind` (`execution.md` §3.4) immediately, with no partial state mutation observable beyond `gas_used` (`execution.md` §3.2), when:

1. **Bytecode decode failure:** the next byte at the instruction pointer does not match any opcode defined in §4, or a fixed-size immediate operand runs past the end of the code `Cell`'s data bits (`MalformedCell`, §3.5.4).
2. **Stack arity violation:** an opcode requires more operand-stack items than are present, or a `PICK`/`ROLL` `depth` operand is not a valid index into the current stack (`MalformedCell`, §3.5.4).
3. **Reference operand out of range:** a `ref_index` operand (§4.5) names a child-cell reference the current code `Cell` does not have (`MalformedCell`, §3.5.4).
4. **Arithmetic overflow:** an unsigned/signed `ADD`/`SUB`/`NEG`/`MUL`/`DIVMOD`/`CONV`/`STBITS` result does not fit its declared width, or `DIVMOD`'s divisor is zero (`IntegerOverflow`, §3.3, §4.3).
5. **Pruned-branch content access:** `CTOS` is applied to a pruned special `Cell` (`AbsentNode`, §3.5.3).
6. **Cell/slice structural violation:** `LDU`/`LDI` requests more bits than a `Slice` has remaining, `LDREF` requests a reference a `Slice` does not have remaining, `SUBBYTES` requests a range outside its `Bytes` operand, or `STBITS`/`STREF`/`STBYTES` would grow a `Builder` past `state-model.md`'s 128-byte/4-reference limits (`MalformedCell`).
7. **Cryptographic shape violation:** `CHKSIGNU`'s `pubkey` or `signature` operand is not exactly 32 or 64 bytes respectively (`TypeMismatch`).
8. **Gas exhaustion:** debiting the next instruction's gas cost (§4) would exceed the supplied limit (`OutOfGas`, at that exact instruction, per `execution.md` §3.4).
9. **Explicit throw:** `THROW` always raises its named kind (§4.5); this is normal control flow, not a fault, but is listed for completeness since it is the only opcode whose entire effect is raising an exception.

---

## 6. Test plan

1. **Opcode round-trip tests:** every opcode in §4 encodes and decodes to the same instruction; an unrecognized opcode byte raises `MalformedCell`.
2. **Arithmetic flavor tests:** for each of `ADD`/`SUB`/`NEG`/`MUL`/`DIVMOD`, unsigned and signed flavors raise `IntegerOverflow` exactly at the documented width boundary; the modulo flavor does not raise it there and instead produces the correctly-wrapped bit pattern. `DIVMOD` with a zero divisor raises `IntegerOverflow` regardless of flavor.
3. **Conversion tests:** `CONV` accepts values that fit the target width/signedness and raises `IntegerOverflow` for values that don't, at both the unsigned and signed boundaries.
4. **Bit-string/byte-string tests:** `CONCAT`/`SUBBYTES`/`BYTEEQ`/`BYTELEN` round-trip correctly; `SUBBYTES` with an out-of-range `offset`/`len` raises `MalformedCell`.
5. **Cell/slice tests:** a `Builder` built via `NEWC`/`STBITS`/`STREF`/`STBYTES`/`ENDC` round-trips through `CTOS`/`LDU`/`LDI`/`LDREF` to the original values; exceeding 128 bytes or 4 references during `ST*` raises `MalformedCell`; reading past a `Slice`'s remaining bits/refs raises `MalformedCell`.
6. **Pruned-branch tests:** `CTOS` on a pruned special `Cell` raises `AbsentNode`; `LDREF` and `HASHCELL` on/of the same pruned `Cell` do not raise anything and return the reference/hash respectively (§3.5.3).
7. **Control-flow tests:** `JMPREF`/`CALLREF`/`RET`/`IFJMPREF`/`IFNOTJMPREF`/`IFCALLREF`/`IFNOTCALLREF` correctly transfer control per §4.5; an out-of-range `ref_index` raises `MalformedCell` only when the branch is actually taken; `RET` with an empty call stack terminates execution successfully; `THROW` raises exactly the requested kind for each of its four valid operand values.
8. **Gas accounting tests:** total `gas_used` after a run equals the sum of each executed instruction's §4 cost; exhausting the limit mid-run raises `OutOfGas` at the exact instruction that would exceed it (cross-references `execution.md` §6's determinism and gas tests).
9. **`MAX_TENTATIVE_GAS` plausibility test:** a representative External Inbound admission check (`LDREF`/`LDU` cell reads plus one `CHKSIGNU`) consumes strictly less than `transactions.md`'s `MAX_TENTATIVE_GAS = 10,000` (§3.6).
