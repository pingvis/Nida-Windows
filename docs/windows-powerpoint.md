# PowerPoint inputs on Windows

Install and activate the desktop version of Microsoft PowerPoint, then choose **Add Input → PowerPoint → Choose a deck…** or drop a `.pptx` onto Nida. Each file becomes its own input. **Input Settings → + Add layer → PowerPoint** adds it as a layer.

PowerPoint renders the slideshow in a borderless window that Nida captures through Windows Graphics Capture. Leave that window open and unminimized. Animations and embedded videos are rendered by PowerPoint.

Use the input's arrow buttons or PageDown/PageUp to advance slides and animation builds. B toggles black. First, last, black, white, restart, speaker notes, presenter view and video controls use the same Nida interface as on macOS. At the last animation, Next holds a black end screen; Previous restores the final slide.

PowerPoint plays slide audio through the Windows output device. Route that device's loopback or an appropriate audio input into Nida's mixer if the programme needs it. The macOS per-app audio capture button is not offered on Windows.

Decks opened by Nida are read-only and closed without saving when the input is removed. Decks that were already open remain open. PowerPoint's normal Protected View or activation dialogs may require attention in PowerPoint.

No Office suite, Node runtime or separate PPTX renderer is included. Nida uses PowerPoint itself.

The Windows PowerShell helper is embedded in the executable; the portable ZIP and Inno installer need no extra script files. It runs in a persistent STA process with bounded command waits. It never evaluates deck paths as script code.

## Validation

CI runs the command tests with Windows PowerShell 5.1, compiles the Windows application, and runs the presentation worker and helper transport tests without requiring Office.

A final desktop check needs installed PowerPoint: import a deck containing hidden slides, an animation build, notes and an embedded video; verify preview/program capture, Next/Previous, B, first/last, video pause/seek, removal, show reopening and mixed-DPI monitor placement. Hosted GitHub runners do not include Office.
