# oxidizr-e2e

The end-to-end regression harness: drives `oxidizr-api::execute`
in-process over the shared corpus under `e2e/corpus/` and asserts
use-cases only — exit class, the OXZ code set, and structural counts
from the blessable `workspace.e2e.json` sidecars.

Never bytes, never message text: those may change; the codes and shapes
are the contract (byte-parity lives in the `testdata/` goldens).
Hermetic by construction: empty env map, no stdin, no rendering.

`cargo xtask e2e bless` regenerates the sidecars; the git diff of a bless
is the review artifact.
