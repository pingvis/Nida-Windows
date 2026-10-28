# Embedded by windows.rs. One STA owns Office COM objects for its lifetime.
# JSON is the only input protocol; deck paths are never evaluated as code.
param([switch]$DefineOnly)
$ErrorActionPreference = 'Stop'
$script:powerpoint = $null
$script:ended = @{}
$script:showWindows = @{}

function Get-PowerPoint {
    param([switch]$Create)
    if ($null -ne $script:powerpoint) {
        try { $null = $script:powerpoint.Presentations.Count; return $script:powerpoint }
        catch { $script:powerpoint = $null }
    }
    if ($null -eq [Type]::GetTypeFromProgID('PowerPoint.Application')) { throw 'notinstalled' }
    try { $script:powerpoint = [Runtime.InteropServices.Marshal]::GetActiveObject('PowerPoint.Application') }
    catch {
        if (!$Create) { throw 'notrunning' }
        $script:powerpoint = New-Object -ComObject PowerPoint.Application
    }
    return $script:powerpoint
}

function Convert-DeckPath([string]$path) {
    # Rust canonicalize uses extended-length Windows paths; Office FullName
    # uses ordinary drive/UNC paths. Compare and open the same representation.
    $path = $path.Replace('/', '\')
    if ($path.StartsWith('\\?\UNC\', [StringComparison]::OrdinalIgnoreCase)) {
        return '\\' + $path.Substring(8)
    }
    if ($path.StartsWith('\\?\', [StringComparison]::OrdinalIgnoreCase)) {
        return $path.Substring(4)
    }
    return $path
}

function Find-Deck($app, [string]$path) {
    $path = Convert-DeckPath $path
    foreach ($deck in $app.Presentations) {
        if ([string]::Equals((Convert-DeckPath ([string]$deck.FullName)), $path, [StringComparison]::OrdinalIgnoreCase)) {
            return $deck
        }
    }
    throw 'nopres'
}

function Get-Show($deck) {
    try {
        $window = $deck.SlideShowWindow
        if ($null -ne $window) { $null = $window.View.State; return $window }
    } catch {}
    throw 'noshow'
}

function Get-ShowScreen($deck, $view) {
    if ($script:ended[[string]$deck.FullName]) { return 'done' }
    switch ([int]$view.State) {
        3 { return 'black' }
        4 { return 'white' }
        5 { return 'done' }
        default { return 'running' }
    }
}

function Get-Step($deck, $window) {
    $view = $window.View
    $index = [int]$view.Slide.SlideIndex
    return ("step", $index, [int]$view.CurrentShowPosition, (Get-ShowScreen $deck $view)) -join "`t"
}

function Get-ShowHwnd($window) {
    $path = [string]$window.Presentation.FullName
    $cached = $script:showWindows[$path]
    if ($null -ne $cached -and [NidaPowerPointNative]::IsWindow($cached)) { return $cached }
    # SlideShowWindow has no HWND in some Office versions. Activating this
    # COM show identifies its native slideshow window without localized
    # title prefixes. Cache it so polling never changes keyboard focus.
    $window.Activate()
    $pidOfApp = 0
    $appHwnd = [NidaPowerPointNative]::Handle([int]$window.Application.HWND)
    if ($appHwnd -ne [IntPtr]::Zero) {
        $null = [NidaPowerPointNative]::GetWindowThreadProcessId($appHwnd, [ref]$pidOfApp)
    }
    $file = [IO.Path]::GetFileName($path)
    $deadline = [DateTime]::UtcNow.AddSeconds(2)
    do {
        $hwnd = [NidaPowerPointNative]::FindSlideShow($file, $pidOfApp)
        if ($hwnd -ne [IntPtr]::Zero) {
            $script:showWindows[$path] = $hwnd
            return $hwnd
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'PowerPoint started its show, but its native window could not be identified'
}

function Get-Reading($deck) {
    try { $window = Get-Show $deck }
    catch { return ("noshow", [int]$deck.Slides.Count) -join "`t" }
    $view = $window.View
    $rect = New-Object NidaPowerPointNative+Rect
    if (![NidaPowerPointNative]::GetWindowRect((Get-ShowHwnd $window), [ref]$rect)) {
        throw 'Cannot read the PowerPoint slide show window'
    }
    $kind = if ([int]$window.IsFullScreen -eq -1) { 'full' } else { 'window' }
    return ("show", [int]$view.Slide.SlideIndex, [int]$view.CurrentShowPosition,
        [int]$deck.Slides.Count, (Get-ShowScreen $deck $view),
        $rect.Left, $rect.Top, ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top), $kind) -join "`t"
}

function Set-ShowPlacement($window, $at) {
    $hwnd = Get-ShowHwnd $window
    # Only the slide show window is made borderless. Pixel coordinates avoid
    # PowerPoint's point/DPI conversions and match Windows Graphics Capture.
    $style = [NidaPowerPointNative]::GetWindowLong($hwnd, -16)
    $style = $style -band (-bnot (0x00C00000 -bor 0x00040000 -bor 0x00800000))
    $null = [NidaPowerPointNative]::SetWindowLong($hwnd, -16, $style)
    if (![NidaPowerPointNative]::SetWindowPos($hwnd, [IntPtr]::Zero,
        [int]$at.left, [int]$at.top, [Math]::Max(64, [int]$at.width),
        [Math]::Max(64, [int]$at.height), 0x0034)) {
        throw "Cannot position PowerPoint window $hwnd (Office HWND $($window.HWND), Windows error $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))"
    }
}

function Move-Show($deck, $window, $to) {
    $key = [string]$deck.FullName
    $view = $window.View
    $wasEnded = $script:ended[$key]
    if ($wasEnded -and $to -ne 'next') { $view.State = 1 }
    if ($to -isnot [string]) {
        $target = [Math]::Max(1, [Math]::Min([int]$deck.Slides.Count, [int]$to.slide))
        $view.GotoSlide($target)
        $script:ended.Remove($key)
    } else {
        switch ($to) {
            'next' {
                if (!$script:ended[$key]) {
                    $last = [int]$deck.Slides.Count
                    while ($last -gt 1 -and [int]$deck.Slides.Item($last).SlideShowTransition.Hidden -eq -1) { $last-- }
                    # Keep a captured black end screen instead of letting
                    # PowerPoint close the window on the final Next.
                    if ([int]$view.Slide.SlideIndex -ge $last -and
                        ($view.GetClickCount() -eq 0 -or $view.GetClickIndex() -eq -2 -or $view.GetClickIndex() -ge $view.GetClickCount())) {
                        $view.State = 3
                        $script:ended[$key] = $true
                    } else { $view.Next() }
                }
            }
            'previous' {
                if ($script:ended[$key]) { $view.State = 1 }
                else { $view.Previous() }
                $script:ended.Remove($key)
            }
            'first' { $view.First(); $script:ended.Remove($key) }
            'last' { $view.Last(); $script:ended.Remove($key) }
            default { throw 'Unknown PowerPoint navigation command' }
        }
    }
    return Get-Step $deck $window
}

function Get-Video($deck, $window, $request) {
    $view = $window.View
    if ((Get-ShowScreen $deck $view) -eq 'done') { return "video`t0`t0" }
    $slide = $view.Slide
    $index = [int]$slide.SlideIndex
    $shape = 0
    if ($null -ne $request.known -and [int]$request.known[0] -eq $index) {
        $shape = [int]$request.known[1]
    }
    if ($shape -eq 0) {
        for ($i = 1; $i -le $slide.Shapes.Count; $i++) {
            if ([int]$slide.Shapes.Item($i).Type -eq 16) { $shape = $i; break }
        }
    }
    if ($shape -eq 0) { return "video`t$index`t0" }
    # Player takes a shape ID, which differs from its 1-based collection index.
    $player = $view.Player([int]$slide.Shapes.Item($shape).Id)
    switch ($request.control) {
        'play' { $player.Play() }
        'pause' { $player.Pause() }
        'seek' { $player.CurrentPosition = [int]$request.position }
        'restart' { $player.CurrentPosition = 0; $player.Play() }
    }
    $state = switch ([int]$player.State) {
        0 { 'playing' }
        1 { 'paused' }
        2 { 'stopped' }
        default { 'not ready' }
    }
    return ("video", $index, $shape, $state, [int]$player.CurrentPosition) -join "`t"
}

function Invoke-PowerPointCommand($request) {
    if ($request.op -eq 'ping') { return 'ready' }
    if ($request.op -eq 'refocus') {
        $front = [NidaPowerPointNative]::GetForegroundWindow()
        $pidOnTop = 0
        $null = [NidaPowerPointNative]::GetWindowThreadProcessId($front, [ref]$pidOnTop)
        $process = Get-Process -Id $pidOnTop -ErrorAction SilentlyContinue
        if ($null -ne $process -and $process.ProcessName -ieq 'POWERPNT') {
            $nida = Get-Process -Id ([int]$request.pid) -ErrorAction SilentlyContinue
            if ($null -ne $nida -and $nida.MainWindowHandle -ne [IntPtr]::Zero) {
                $null = [NidaPowerPointNative]::SetForegroundWindow($nida.MainWindowHandle)
            }
        }
        return 'focused'
    }
    if ($request.op -eq 'slide_shows') {
        try { $app = Get-PowerPoint }
        catch { if ($_.Exception.Message -eq 'notrunning') { return 'notrunning' }; throw }
        return "shows`t$([int]$app.SlideShowWindows.Count)"
    }
    $app = Get-PowerPoint -Create:($request.op -eq 'open')
    $path = Convert-DeckPath ([string]$request.path)
    if ($request.op -eq 'open') {
        try { $null = Find-Deck $app $path; return 'opened' }
        catch { if ($_.Exception.Message -ne 'nopres') { throw } }
        if ([int]$app.Visible -ne -1) { $app.Visible = -1 }
        $security = $app.AutomationSecurity
        try {
            $app.AutomationSecurity = 3
            $null = $app.Presentations.Open($path, -1, 0, -1)
        } finally { $app.AutomationSecurity = $security }
        return 'opened'
    }
    $deck = Find-Deck $app $path
    switch ($request.op) {
        'save_pdf' {
            if ([int]$app.SlideShowWindows.Count -gt 0) { return 'showing' }
            $out = Convert-DeckPath ([string]$request.out)
            # PDF copy, all slides (including hidden), leaving the deck unchanged.
            # ExportAsFixedFormat requires a null PrintRange for all slides.
            $deck.ExportAsFixedFormat($out, 2, 1, 0, 1, 1, -1, $null, 1)
            return 'saved'
        }
        'read' { return Get-Reading $deck }
        'window' {
            $hwnd = Get-ShowHwnd (Get-Show $deck)
            return [string]$hwnd.ToInt64()
        }
        'start' {
            $settings = $deck.SlideShowSettings
            $settings.ShowType = 2
            $settings.ShowScrollbar = 0
            $settings.ShowPresenterView = 0
            $settings.LoopUntilStopped = 0
            $script:showWindows.Remove($path)
            $window = $settings.Run()
            Set-ShowPlacement $window $request.at
            $script:ended.Remove($path)
            if ($null -ne $request.slide -and [int]$request.slide -gt 1) {
                $window.View.GotoSlide([Math]::Min([int]$request.slide, [int]$deck.Slides.Count))
            }
            return Get-Reading $deck
        }
        'place' {
            $window = Get-Show $deck
            Set-ShowPlacement $window $request.at
            return Get-Reading $deck
        }
        'go' { return Move-Show $deck (Get-Show $deck) $request.to }
        'screen' {
            $window = Get-Show $deck
            $window.View.State = switch ($request.screen) { 'black' { 3 } 'white' { 4 } default { 1 } }
            $script:ended.Remove($path)
            return Get-Step $deck $window
        }
        'end' {
            try { (Get-Show $deck).View.Exit() } catch { if ($_.Exception.Message -ne 'noshow') { throw } }
            $script:ended.Remove($path)
            $script:showWindows.Remove($path)
            return 'ended'
        }
        'close' {
            # The manager calls close only for decks Nida opened read-only.
            $deck.Saved = -1
            $deck.Close()
            $script:ended.Remove($path)
            $script:showWindows.Remove($path)
            return 'closed'
        }
        'video' {
            try { $window = Get-Show $deck } catch { return "video`t0`t0" }
            return Get-Video $deck $window $request
        }
        default { throw 'Unknown PowerPoint operation' }
    }
}

if ($DefineOnly) { return }

try {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Collections.Generic;
using System.Text;
public static class NidaPowerPointNative {
    public static IntPtr Handle(int hwnd) { return new IntPtr(unchecked((long)(uint)hwnd)); }
    private delegate bool WindowCallback(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(WindowCallback callback, IntPtr data);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr hwnd, StringBuilder name, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
    public static IntPtr FindSlideShow(string filename, int pid) {
        var named = new List<IntPtr>();
        EnumWindows(delegate(IntPtr hwnd, IntPtr data) {
            var name = new StringBuilder(256);
            GetClassName(hwnd, name, name.Capacity);
            bool slideClass = String.Equals(name.ToString(), "screenClass", StringComparison.OrdinalIgnoreCase);
            bool frameClass = String.Equals(name.ToString(), "PPTFrameClass", StringComparison.OrdinalIgnoreCase);
            if (!slideClass && !frameClass) return true;
            int owner;
            GetWindowThreadProcessId(hwnd, out owner);
            if (pid != 0 && owner != pid) return true;
            try {
                if (!String.Equals(System.Diagnostics.Process.GetProcessById(owner).ProcessName, "POWERPNT", StringComparison.OrdinalIgnoreCase)) return true;
            } catch { return true; }
            var title = new StringBuilder(2048);
            GetWindowText(hwnd, title, title.Capacity);
            bool hasName = title.ToString().IndexOf(filename, StringComparison.OrdinalIgnoreCase) >= 0;
            if (slideClass || (hasName && hwnd == GetForegroundWindow()) ||
                title.ToString().IndexOf("PowerPoint Slide Show", StringComparison.OrdinalIgnoreCase) >= 0) {
                if (hasName) named.Add(hwnd);
            }
            return true;
        }, IntPtr.Zero);
        IntPtr front = GetForegroundWindow();
        if (named.Contains(front)) return front;
        return named.Count == 1 ? named[0] : IntPtr.Zero;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll", EntryPoint="GetWindowLongW")] public static extern int GetWindowLong(IntPtr hwnd, int index);
    [DllImport("user32.dll", EntryPoint="SetWindowLongW")] public static extern int SetWindowLong(IntPtr hwnd, int index, int value);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out int pid);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
    # PMv2 gives physical desktop pixels for GetWindowRect/SetWindowPos on
    # mixed-DPI monitors. PowerPoint still draws its own slide at that size.
    $null = [NidaPowerPointNative]::SetThreadDpiAwarenessContext([IntPtr](-4))
    while ($null -ne ($line = [Console]::ReadLine())) {
        try {
            $request = $line | ConvertFrom-Json
            $reply = Invoke-PowerPointCommand $request
            $response = @{ reply = [string]$reply }
        } catch {
            $message = $_.Exception.Message
            if ($message -match '0x80048240|0x80010001|0x8001010A') {
                $message += '. Close PowerPoint dialogs and check that desktop PowerPoint is activated.'
            }
            $code = if ($message -in @('notinstalled', 'notrunning', 'nopres', 'noshow')) { $message } else { 'failed' }
            $response = @{ error = $code; message = $message }
        }
        [Console]::WriteLine(($response | ConvertTo-Json -Compress))
        # Release temporary RCWs; the application reference remains rooted.
        $request = $null
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
    }
} catch {
    [Console]::WriteLine((@{ error = 'failed'; message = $_.Exception.Message } | ConvertTo-Json -Compress))
}
