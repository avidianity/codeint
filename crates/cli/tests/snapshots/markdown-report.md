# codeint report

codeint measured 1 file with 7 source lines.

Test gap is not measured because no Coverage report was named.

| Source set | Metric | Value | Behind the value | Reference range |
| ---------- | ------ | ----- | ---------------- | --------------- |
| Production | Erosion | 0 | 0 of 5.2915 Function mass in eroded functions | 0.187 to 0.671, source: codeint stratified panel of 100 TypeScript and JavaScript repositories, ADR 0035, codeint 0.8.0 |
| Production | Cognitive complexity | 0 | 0 of 2 cognitive complexity in functions over 15 | 0.121 to 0.532, source: codeint stratified panel of 100 TypeScript and JavaScript repositories, ADR 0035, codeint 0.8.0 |
| Production | Duplication | 0 | 0 clone lines of 7 source lines | 0.018 to 0.252, source: codeint stratified panel of 100 TypeScript and JavaScript repositories, ADR 0035, codeint 0.8.0 |
| Production | Verbosity | 0.1429 | 1 Flagged line of 7 source lines | 0 to 0.002, source: codeint stratified panel of 100 TypeScript and JavaScript repositories, ADR 0035, codeint 0.8.0 |
| Production | Import cycles | 0 | 0 of 1 module in an Import cycle | 0 to 0.182, source: codeint stratified panel of 100 TypeScript and JavaScript repositories, ADR 0035, codeint 0.8.0 |

## Production Findings

### Verbosity

| Flagged line | Rule |
| ------------ | ---- |
| `src/sign.ts:4` | else-after-return |

## Files not measured

- `src/broken.ts`: cannot parse: Unexpected token at line 1, column 14
- `src/types.d.ts`: declaration only
