# Native text: our review branch versus Nida 0.9.7

Read-only source comparison, 1 November 2026. No implementation was merged,
no application installed/launched and no PR opened or closed for this audit.

Compared exact revisions:

- Upstream release `v0.9.7`, `45e6f0b95741bfd15e9cda16e1284cdf2096f2fd`.
  [Release notes](https://github.com/kristijonasatpro/Nida/releases/tag/v0.9.7).
  Published 1 November 2026, 13:53:52 Europe/Vilnius.
- Our local review commit `b726d6d0781232d6911addf9dbf4f17740fc9640`, branch
  `codex/native-text-inputs`. Its scope/previous Windows validation is recorded
  in [native-text-inputs.md](native-text-inputs.md).

## Findings

Upstream covers the requested static, one-style-per-input, live-editing use
case, and adds a larger conference graphics workflow. Adopt its implementation
as the future base; keep selected improvements from our branch as focused
follow-ups rather than submitting a second complete Text implementation.

| Area | Our branch | Upstream 0.9.7 |
| --- | --- | --- |
| Basic titles | Multiline static text, installed fonts, bold/italic, alignment, nine anchors | Same core use case |
| Editor | Separate window, auto-open after creation, Edit text buttons | Integrated into Input Settings |
| Line layout | Wrapping to a chosen fraction of output width; fixed line spacing | Explicit newlines; adjustable line spacing; box hugs measured ink; very large drawings reduced toward the 8192-pixel side cap |
| Look | RGBA text, background and configurable coloured offset shadow; outline; fixed padding | RGB text, box colour/opacity, adjustable padding/rounded corners, outline, soft blurred black shadow, Lithuanian font-coverage warning |
| Visibility | Existing channel/input/overlay controls | Dedicated text Shown/fade controls, plus Show on selected buses and layout for their LED-screen regions |
| Name rosters | Static text entered manually | Separate Lower third source: name plus two title lines, CSV/TSV reload, organiser-friendly encodings, seven styles, 12 palettes, image plates, four transition types and a dockable show-wide panel |
| Undo | A text command per edit; slider gestures grouped | A typing run grouped into one undo step; explicit handling of undo/API changes arriving while typing |
| Render status | Snapshot/UI pending and error; invalid style values rejected | Snapshot contains settings; values normalized/clamped, text truncated to 4000 characters; draw errors logged |
| Existing validation | Windows debug/GPU tests from our review | Released macOS application; no Windows build or runtime qualification performed by this audit |

Both implementations rasterize text with the existing CPU SVG/font stack on a
worker when content/style/resolution changes, upload the image and composite
on the GPU. Neither is an all-GPU font renderer, and neither needs a general
source-provider migration first. Both retain the prior picture while painting.

## Live editing and performance details

Our mailbox strictly bounds pending work to one request and one result, and
reuses the GPU texture when dimensions stay the same. Upstream uses unbounded
MPSC channels, drains to the latest request/result and creates new GPU texture
resources for each accepted drawing. Its pictures are tightly cropped, whereas
ours uses the chosen wrapping-box width. No speed or memory winner has been
measured across implementations.

There is a meaningful weakness in our "latest edit only" policy: the worker
drops a completed result if a newer request is pending, and tick also rejects
any result older than the latest requested revision. From source inspection,
continuous edits arriving faster than paint can finish can therefore hold the
old title until the worker catches up. Upstream deliberately displays
progressively newer completed pictures while continuing to coalesce requests,
and includes a regression test for that exact fast-typing case. Preserve that
behavior when improving its queue bounds.

Upstream also explicitly distinguishes pending typing acknowledgements from
external undo/API changes. Our single pending settings value is less capable:
an external change can be ignored while awaiting a different acknowledgement.
This is a source-level finding, not a newly reproduced runtime failure.

Our remaining useful deltas are wrapping/width, explicit pending/error status,
strict content/allocation bounds, same-size texture reuse and configurable RGBA
text/shadow colours. Upstream caps a side at 8192; our bitmap has an additional
16-megapixel area cap. Our renderer never falls back to rasterizing on the
frame thread if painter creation fails; upstream does, logging a warning.

## Saved-show and API compatibility

Both use source kind/type `text` and commands named AddTextDeck and
SetTextSettings, but the settings are incompatible:

- Ours has flat `font`, `size`, `color`, `width`, `anchor`, `background`, etc.
- Upstream has nested `look` (`colour`, `box_opacity`, `spacing`, etc.), plus
  `visible`, `fade_secs` and `targets`.

Reading our review show in upstream without conversion would preserve the
shared `text` field but ignore most flat style fields and use defaults. This
could move/re-style the title silently. A migration should preserve UUIDs and
map the style explicitly; wrapping width, coloured shadow and independent text
alpha have no direct upstream equivalent. Do not merge both schemas blindly.

Creation and reads use POST /api/inputs and GET /api/state in both. We add
PUT /api/decks/{deck_uuid}/text; upstream exposes editing through the generic
POST /api/command with SetTextSettings. Our dedicated route could remain as a
thin adapter to the upstream canonical settings.

The text core uses portable Rust/wgpu/SVG. It shares Placer/rendering support
with the new lower-third module and uses portable font-coverage support, so a
Windows integration is broader than copying a single text.rs file. The macOS
release alone does not validate that integration on Windows.

## Primary sources

- [Upstream Text manual](https://github.com/kristijonasatpro/Nida/blob/v0.9.7/docs/23-adding-inputs.md#text).
- [Settings and persistence schema](https://github.com/kristijonasatpro/Nida/blob/v0.9.7/src/engine/value/text.rs#L43).
- [Painter, completed-image handling and upload](https://github.com/kristijonasatpro/Nida/blob/v0.9.7/src/internal/deck/text.rs#L257).
- [Typing state and grouped undo](https://github.com/kristijonasatpro/Nida/blob/v0.9.7/src/usecases/ui/panels/text.rs#L64).
- [Fast-typing regression](https://github.com/kristijonasatpro/Nida/blob/v0.9.7/src/internal/deck/text.rs#L966).
- [Lower-third settings](https://github.com/kristijonasatpro/Nida/blob/v0.9.7/src/engine/value/lower_third.rs#L357).

The pinned upstream snapshot is under target/native-text-097-comparison/upstream.
No new builds, benchmarks or runtime tests were run for this comparison.
