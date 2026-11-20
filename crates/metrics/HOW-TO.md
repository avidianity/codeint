# How to use the built-in Metrics

## Find Clone groups

```rust
use codeint_metrics::duplication::{MinimumCloneTokens, find_clone_groups};

let duplication = find_clone_groups(&model, MinimumCloneTokens::PROVISIONAL_DEFAULT);
println!("{} of {} source lines are clone lines", duplication.clone_lines, duplication.source_lines);
let ratio = duplication.ratio();
```

- `find_clone_groups` reads only the token stream of each file in the `CodeModel`.
- A Clone group holds two or more fragments with the same token sequence, at least the minimum number of tokens long.
  Each fragment is as long as the tokens that all copies share, and copies of one fragment never overlap in one file.
- A clone line is a line on which a token of a fragment starts; `clone_lines` counts each such line once.
- `ratio` is `clone_lines / source_lines`, and 0 when the model has no source lines.
- Clone groups come largest first, then by their first fragment.
- `MinimumCloneTokens::PROVISIONAL_DEFAULT` is 50 tokens until the calibration Ticket #33 fixes the default.
