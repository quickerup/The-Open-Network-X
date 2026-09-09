# ONX Specification — Distributed Hash Table (DHT)

**Status:** Draft  
**Scope:** Peer identity discovery, service registration, ADNL tunnel entry points, Kademlia-like routing, and fall-back keys for Open Network X (ONX).

*Note: This document is the second of three Networking sub-specifications (Architecture sequence item 8, partially resolving ONX-ARCH-010). Transport identity and ADNL packet encodings are defined in `networking-adnl.md`; Overlay Networks and Multicasting are defined in `networking-overlay.md`.*

---

## 1. Reference

- `whitepaper.md`, §3.2: TON DHT: Kademlia-like Distributed Hash Table (§3.2.1–§3.2.12).
  - §3.2.1: 256-bit keys, TL key descriptions, and preimages.
  - §3.2.2–§3.2.4: XOR distance metric, $k$-buckets, and iterative lookup algorithm.
  - §3.2.5–§3.2.10: Record types (node address, service description, tunnel entry point).
  - §3.2.11: Fall-back keys and anti-censorship mechanisms.
  - §3.2.12: Signed DHT values, owner validation, and expiry bounds.
- `docs/specification/networking-adnl.md`: Abstract addresses, key descriptions, ADNL/RLDP transport.
- `docs/specification/protocol-primitives.md`: SHA-256 digests, Ed25519 signatures, big-endian integer encodings.
- `INSTRUCTIONS.md`, §9, §13: Separation of network discovery from consensus validity.

---

## 2. Requirement

The ONX network requires a decentralized lookup mechanism to discover:
1. Contact information (IP address and port) associated with a given 256-bit ADNL abstract address.
2. ADNL proxy tunnel entry points and intermediate relay nodes for ONX Proxy routing.
3. Network service endpoints (such as public RPC nodes, storage hosts, or bridge gateways).

This lookup mechanism must be resilient against eclipse attacks, key retention attacks, and data poisoning, without allowing unauthenticated or malicious discovery entries to affect consensus decisions.

---

## 3. ONX Interpretation

### 3.1 256-Bit Keys and Distance Metric

- All DHT keys and node IDs are 256-bit unsigned integers (`uint256`).
- A DHT key $K$ is derived strictly as the SHA-256 digest of a canonical TL-serialized **key description** object:
  $$K = \text{SHA-256}(\text{key\_description})$$
- Distance between two 256-bit integers $A$ and $B$ is defined by the Kademlia XOR metric:
  $$d(A, B) = A \oplus B$$
- Node routing tables are organized into 256 $k$-buckets, where bucket $i$ ($0 \le i < 256$) contains contact details for up to $k = 20$ active nodes whose XOR distance from the local node ID lies in the range $[2^i, 2^{i+1} - 1]$.

### 3.2 DHT Record Kinds

ONX DHT defines four canonical record types ($V$), identified by a 4-byte constructor tag:

1. **Node Contact Record (`0x21d803a1`):** Maps an abstract ADNL address to an IP address (IPv4 or IPv6) and UDP/TCP transport port.
2. **Tunnel Entry Record (`0x34a19b02`):** Maps an abstract tunnel ID to an active ADNL proxy endpoint.
3. **Service Registration Record (`0x4f880c10`):** Maps a 256-bit service ID to a list of advertised provider endpoints.
4. **Account Pointer Record (`0x51b033d9`):** Maps a 256-bit account address or domain hash to an abstract ADNL address.

### 3.3 Record Authentication and Expiry

- Every stored DHT record MUST be signed by the private key corresponding to the record's owner address (`owner_address`).
- A record includes an explicit `expiry` timestamp (UNIX Epoch seconds). Nodes MUST NOT store or return records whose `expiry` is in the past.
- The maximum permitted lifetime for any DHT record is $86,400$ seconds (24 hours). Attempts to store records with longer expiry periods MUST be rejected or truncated to the maximum lifetime.

### 3.4 Fall-Back Keys and Anti-Censorship

- To prevent key retention and censorship attacks (where an adversary floods node IDs numerically close to a target key $K$ to eclipse lookups), key descriptions include a 32-bit `extra_id` field (`uint32`).
- The primary DHT key uses `extra_id = 0`.
- If lookup or store operations on key $K_0 = \text{SHA-256}(\text{key\_description with extra\_id = 0})$ fail due to key retention or eclipse, the node increments `extra_id` to $1, 2, \dots$ to generate fall-back keys $K_1, K_2, \dots$.
- Querying nodes probe $K_0$ first, falling back to $K_1 \dots K_m$ until a valid signed record is retrieved.

### 3.5 Discovery vs. Consensus Boundary

- DHT lookup responses are strictly **advisory operational data**.
- A node MUST NOT alter block validity, consensus votes, validator membership, or transaction execution based on DHT query results.
- Validator set composition and validator pubkeys are derived exclusively from Masterchain state, never from DHT records.

---

## 4. Serialization

All fields are encoded in big-endian byte order.

### 4.1 DHT Value Header and Payload Layout

```
DhtRecord:
  1. key             : uint256 (32 bytes - SHA-256 of key_description)
  2. record_kind     : uint32  (Constructor tag: 0x21d803a1, 0x34a19b02, 0x4f880c10, or 0x51b033d9)
  3. expiry          : uint64  (UNIX Epoch seconds timestamp)
  4. owner_address   : uint256 (32 bytes - ADNL abstract address of creator)
  5. value_len       : uint32  (length of payload bytes)
  6. value_bytes     : bytes   (value_len bytes containing the record-kind payload)
  7. signature       : bytes   (64 bytes Ed25519 signature over items 1..6)
```

### 4.2 Record Payload Formats

#### Node Contact Record (`record_kind = 0x21d803a1`)
```
NodeContactValue:
  1. ip_version      : uint8   (4 for IPv4, 6 for IPv6)
  2. ip_address      : bytes   (4 bytes for IPv4, 16 bytes for IPv6)
  3. udp_port        : uint16
  4. tcp_port        : uint16
  5. key_description : bytes   (TL-serialized KeyDescription)
```

#### Tunnel Entry Record (`record_kind = 0x34a19b02`)
```
TunnelEntryValue:
  1. target_address  : uint256 (32 bytes destination ADNL address)
  2. tunnel_id       : uint256 (32 bytes channel/tunnel identifier)
  3. key_description : bytes   (TL-serialized KeyDescription)
```

### 4.3 DHT RPC Query Encodings

DHT queries are transmitted over RLDP/ADNL:

- `DhtPing (0x6a10021f)`: Payload = `node_id:uint256`. Response = `DhtPong (0x1b40920a)`.
- `DhtFindNode (0x74c98200)`: Payload = `target_id:uint256 | k:uint32`. Response = list of $k$ closest `NodeContactRecord`s.
- `DhtFindValue (0x81e2b405)`: Payload = `key:uint256`. Response = `DhtValue` if present, else list of $k$ closest nodes.
- `DhtStoreValue (0x9320ab11)`: Payload = `DhtRecord`. Response = `DhtStoreAck (0x02a11894)`.

---

## 5. Malformed-input behavior

Nodes MUST reject and drop DHT messages or records immediately if any of the following occur:

1. **Signature Verification Failure:** The signature on `DhtRecord` does not verify against the public key extracted from `owner_address`'s key description.
2. **Key Digest Mismatch:** `key` in `DhtRecord` does not match SHA-256 of the record's key description or payload description.
3. **Expired Record:** `expiry` timestamp is less than or equal to current node system time.
4. **Excessive Lifetime:** `expiry` timestamp exceeds local system time by more than $86,400$ seconds.
5. **Unknown Record Kind:** `record_kind` tag is not one of the four specified constructor tags (`0x21d803a1`, `0x34a19b02`, `0x4f880c10`, `0x51b033d9`).
6. **Truncated Value:** `value_len` does not match the actual remaining payload length before the signature field.
7. **Kademlia Bucket Overflow:** Attempts by an unauthenticated remote node to flood a $k$-bucket past capacity $k=20$ without passing ping liveness verification.

---

## 6. Test plan

1. **XOR Metric and Routing Table Tests:**
   - Verify XOR distance calculation properties: symmetry $d(A,B) = d(B,A)$, identity $d(A,A) = 0$, triangle inequality $d(A,C) \le d(A,B) \oplus d(B,C)$.
   - Verify $k$-bucket insertion, eviction of unresponsive nodes, and bucket splitting logic.
2. **Iterative Lookup Convergence Tests:**
   - Simulate a network of 100 DHT nodes; verify lookup convergence to the $s=8$ closest nodes within $\log_2(N)$ routing steps.
3. **Record Signing & Validation Tests:**
   - Round-trip serialization and signature verification for all four DHT record kinds.
   - Verify rejection when record payload or `expiry` timestamp is modified post-signing.
4. **Fall-Back Key Tests:**
   - Simulate key retention attack on $K_0$; verify automatic failover to $K_1$ and $K_2$ fall-back keys.
5. **Adversarial Input Tests:**
   - Inject expired records, invalid signatures, malformed payload lengths, and unknown record tags; verify silent drop without node panic.
