# Test Vectors

**Parent:** [../canonical-text-format.md](../canonical-text-format.md)
**Narrative reference:** [../test-vectors.md](../test-vectors.md)

Machine-consumable form of the Stage 2 test vectors. Each file contains exactly the bytes implementations should treat as the vector payload — no trailing newline, no wrapping, no metadata.

The narrative doc (`../test-vectors.md`) is the authoritative description: it explains the pressure-test, the authoring intent, the DeBruijn trace, and the ADR each vector backs. This directory is the cross-implementation fixture set that two canonicalizers run against to demonstrate byte-equivalence.

## File-naming scheme

```
NN-slug.<role>
```

- `NN` — vector number from the narrative doc. Sub-vectors use `10a`, `10b`, …; their parent `10` is not a file.
- `slug` — short hyphenated description. Not load-bearing; grep against the narrative if unsure.
- `<role>` — see below.

## Roles

| Role         | Meaning                                                                                                                        |
|--------------|--------------------------------------------------------------------------------------------------------------------------------|
| `canonical`  | Valid canonical text. Implementations must emit these bytes for the corresponding AST and must round-trip (parse → re-emit → compare byte-identical). |
| `forbidden`  | Syntactically well-formed but spec-forbidden (e.g. `(rec (int 1))` — `rec` with N=0 bindings). Canonicalizers must **never emit**; parsers must refuse the AST. |
| `reject`     | Bytes a compliant parser must reject at lex/parse time (e.g. `\u{d800}` surrogate). Not a hole — a hard error.                 |

No trailing newline in any file. A file whose last byte is `0x0a` has been edited incorrectly.

## How implementations consume this directory

Minimum round-trip test (Stage 2 exit criterion):

1. For every `*.canonical` file: read bytes → parse → re-emit canonical text → assert byte-identical to the original.
2. For every `*.forbidden` file: read bytes → parse succeeds syntactically, but the AST must either be rejected earlier (parser) or refused by the canonicalizer.
3. For every `*.reject` file: read bytes → parser must return a hard error at lex or parse level (not a `(hole ...)` node).

Two independent implementations pass Stage 2 when they agree on (1), (2), and (3) for every file here.

Additional pairwise-distinctness checks (vectors that encode the rule "two inputs produce two outputs"):

- `17a-rec-permuted.canonical` vs `17b-rec-permuted-swapped.canonical` — emitted bytes must differ; hashes must differ.
- `19a-match-wild-first.canonical` vs `19b-match-zero-first.canonical` — same.

Additional parse-and-normalize checks (spec requires specific emission for non-canonical inputs — narrative describes, implementations code up AST construction directly):

- V9 — non-ASCII in `str` emits as `\u{HEX}`.
- V13 — `-0` normalizes to `0`.
- V15 — named-escape preference over `\u{...}`.
- V22 — NUL emits as `\u{0}`, not `\u{00}`.

## Coverage

33 vectors total. V29–V33 are Phase 2 Stage 1 additions; they require the
canonical parser to be extended with Phase 2 tags (`fn-ty`, `ty-var`,
`forall`, `eff-set`, `eff-var`, `pat-int`) before they pass the round-trip
test. V33 uses only existing tags and passes the Phase 1 canonical parser
today.

## Index

| File | Narrative § | Primary ADR |
|------|-------------|-------------|
| `01-identity-lambda.canonical` | V1 | 0005, 0007 |
| `02-let-cascade.canonical` | V2 | 0007 |
| `03-rec-no-lam.canonical` | V3 | 0007 |
| `04-mutual-rec-lam.canonical` | V4 | 0007 |
| `05-record-case-mixed.canonical` | V5 | 0008 |
| `06-nested-records.canonical` | V6 | 0008 |
| `07-pattern-multi-pat-var.canonical` | V7 | spec § 4 |
| `08a-hole-standalone.canonical` | V8 | 0009, spec § 7 |
| `08b-hole-embedded.canonical` | V8 | 0009, spec § 7 |
| `09-string-non-ascii.canonical` | V9 | 0010 S2/S3 |
| `10a-empty-record.canonical` | V10 | 0011 |
| `10b-nullary-ctor.canonical` | V10 | 0011 |
| `10c-empty-module.forbidden` | V10 | 0011 |
| `10d-body-only-rec.forbidden` | V10 | 0011 |
| `10e-zero-arm-match.forbidden` | V10 | 0011 |
| `11-ann-record-type.canonical` | V11 | 0008 |
| `12-proj-nested.canonical` | V12 | spec § 2 |
| `13-signed-zero.canonical` | V13 | 0010 I1 |
| `14-bignum-29-digits.canonical` | V14 | 0010 I2 |
| `15-string-named-escapes.canonical` | V15 | 0010 S1 |
| `16-symbol-regex-edges.canonical` | V16 | spec § 3 |
| `17a-rec-permuted.canonical` | V17 | 0007, spec § 5 |
| `17b-rec-permuted-swapped.canonical` | V17 | 0007, spec § 5 |
| `18-deep-nesting.canonical` | V18 | — |
| `19a-match-wild-first.canonical` | V19 | spec § 6 |
| `19b-match-zero-first.canonical` | V19 | spec § 6 |
| `20a-unexpected-token.canonical` | V20 | spec § 7 |
| `20b-unclosed-paren.canonical` | V20 | spec § 7 |
| `20c-expected-expr.canonical` | V20 | spec § 7 |
| `20d-expected-pattern.canonical` | V20 | spec § 7 |
| `20e-unbound-name.canonical` | V20 | spec § 7 |
| `20f-arity-mismatch.canonical` | V20 | spec § 7 |
| `21-bignum-50-digits.canonical` | V21 | 0010 I2 |
| `22-string-embedded-nul.canonical` | V22 | 0010 S3 |
| `23-string-max-codepoint.canonical` | V23 | 0010 S3 |
| `24a-surrogate-low.reject` | V24 | 0012 |
| `24b-surrogate-high.reject` | V24 | 0012 |
| `24c-out-of-range.reject` | V24 | 0012 |
| `24d-max-6-digit.reject` | V24 | 0012 |
| `24e-just-below-low-surrogate.canonical` | V24 | 0012 |
| `24f-just-above-high-surrogate.canonical` | V24 | 0012 |
| `25-pat-var-inner-lam.canonical` | V25 | spec § 4 |
| `26-ctor-mixed-args.canonical` | V26 | spec § 2 |
| `27-rec-single-binding.canonical` | V27 | 0011 |
| `28-module-one-binding.canonical` | V28 | 0004, 0011 |
| `29-ann-generic-id.canonical` | V29 | 0034, 0035 |
| `30-ann-io-fn.canonical` | V30 | 0034, 0035 |
| `31-ann-eff-poly.canonical` | V31 | 0034, 0035, 0036 |
| `32-pat-int-match.canonical` | V32 | 0037 |
| `33-buf-alloc-read.canonical` | V33 | 0038 |
