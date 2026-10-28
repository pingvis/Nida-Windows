# GitHub Actions in the Windows fork

The fork maintains four workflows:

| Workflow | Trigger | Purpose |
| --- | --- | --- |
| Windows portable and installer | Manual, version tags, selected main changes and packaging pull requests | Build and verify the portable ZIP and installer; save artifacts. |
| Windows build and upload release | Manual | Call the packaging workflow, then replace release downloads or create a draft release. |
| Code checks | Relevant pull requests to main, or manual | Rust formatting, Windows Clippy using the shipped feature set, and the Linux application suite with software Vulkan. |
| Component checks | Pull requests to main, pushes to main, or manual | Select DeckLink, NDI loader, PowerPoint and browser tests using changed files. Manual runs select every component. |

The release workflows share one build implementation. The packaging workflow
also tests CEF extraction/cache handling, real browser pixels, bundled codecs,
and installer install/upgrade/uninstall behavior.

Component checks preserve the isolated browser manager/transport tests and build
their real CEF helper. DeckLink and NDI retain Windows and Linux checks; macOS
jobs and the inherited macOS release workflow have been removed for this fork.
PowerPoint checks run on Windows.

`scripts/ci/component-changes.py` contains the component path filters. It uses
the full Git diff (the merge-base diff for pull requests), so deleted files,
renames and large changes are covered. Documentation-only changes run the small
selector job and skip all component jobs. Dependency and selector changes select
every component. Older workflow names may still appear in GitHub run history.

The consolidation preserves existing formatting and application test gates;
it does not waive pre-existing source failures. Run Code checks manually to
check main without opening a pull request.
