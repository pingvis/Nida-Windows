# Windows chroma-key colour picker

In Input Settings, select the layer (or the channel), open **Key**, enable
**Chroma key**, then choose **Pick colour from preview**. Click the background
in the preview. **Cancel pick** or Escape leaves the colour unchanged.

During a pick the preview shows the picture immediately before that chroma-key
effect, with opaque alpha so a previously keyed background remains visible.
Program, other previews and the effect chain continue normally. Preview view
selection, positioning, mask drawing and browser interaction are suspended for
the pick. Switching inputs, removing/bypassing the effect, or closing Input
Settings cancels the request.

The temporary GPU tap is capped at 960 pixels on its long edge. Its display copy
uses sRGB encoding, while a separate float copy supplies the sampled linear RGB
to the keyer. The edit encodes it into the existing sRGB parameter format; the
normal shader upload decodes it back to the sampled linear value. A single pixel
is read asynchronously; no blocking GPU wait
is added to the application frame loop. The target colour's existing alpha is
preserved. The edit uses the normal effect command, undo history and show
persistence. Capture resources are released when picking ends and are never
written to a show. The macOS system sampler is unchanged.

The picker samples a pixel of the live preview rather than averaging a region.
Fine details smaller than a preview pixel require a larger preview or a closer
source image.

## Validation

Native GPU and UI regression tests cover linear RGB from transparent pixels,
coordinate orientation and edges, cancelled readbacks, selection/effect changes,
alpha preservation, and preview gesture consumption. The real chroma shader is
also exercised: Program remains keyed while the tap exposes the input, and a
midtone survives the complete parameter upload round trip.

On the Windows RTX 3060 Ti test machine, the focused run passed 50 tests, all
14 preview-adjustment tests passed, and all 39 Input Settings tests passed.
Production library/binary Clippy passed. A broader test lint attempt found
pre-existing warnings in unrelated audio and integration tests; those files
were left unchanged.

The debug application was built and launched in an isolated workspace and its
HTTP API accepted and saved the midtone/chroma fixture. Actual desktop clicking
was not verified: the computer-use tool returned **Computer Use app approval
timed out** when requesting the Nida window. This remains a manual verification
step; automated UI gesture tests do not replace it.
