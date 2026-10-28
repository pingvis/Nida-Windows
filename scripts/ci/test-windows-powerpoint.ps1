# Runs with Windows PowerShell 5.1, the runtime embedded by Nida.
$ErrorActionPreference = 'Stop'
$root = Join-Path $PSScriptRoot '../..'
$helper = Join-Path $root 'src/internal/presentation/windows-powerpoint.ps1'
$tokens = $null
$errors = $null
$null = [Management.Automation.Language.Parser]::ParseFile($helper, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw ($errors | Out-String) }
. $helper -DefineOnly

function Assert-Equal($actual, $expected, $label) {
    if ($actual -cne $expected) { throw "$label - expected [$expected], got [$actual]" }
}

# A real path comparison must not confuse two decks sharing a filename.
$unicode = [string][char]0x10c
$path = ('C:\Talks\' + $unicode + ' '' dollar$().pptx')
Assert-Equal (Convert-DeckPath ('\\?\' + $path)) $path 'extended drive path'
Assert-Equal (Convert-DeckPath '\\?\UNC\server\share\talk.pptx') '\\server\share\talk.pptx' 'extended UNC path'
$first = [pscustomobject]@{ FullName = $path }
$second = [pscustomobject]@{ FullName = 'C:\Other\talk.pptx' }
$app = [pscustomobject]@{ Presentations = @($first, $second) }
Assert-Equal (Find-Deck $app $path.ToUpperInvariant()).FullName $path 'case-insensitive full path'
Assert-Equal (Find-Deck $app ('\\?\' + $path)).FullName $path 'canonical Windows path matches Office'
try { $null = Find-Deck $app 'D:\talk.pptx'; throw 'Should reject basename-only matches' }
catch { Assert-Equal $_.Exception.Message 'nopres' 'missing deck' }

$slide = [pscustomobject]@{ SlideIndex = 3 }
$view = [pscustomobject]@{ Slide = $slide; CurrentShowPosition = 3; State = 1; Click = 0; Clicks = 1; Advances = 0 }
$view | Add-Member ScriptMethod GetClickIndex { return $this.Click }
$view | Add-Member ScriptMethod GetClickCount { return $this.Clicks }
$view | Add-Member ScriptMethod Next { $this.Advances++; $this.Click++ }
$view | Add-Member ScriptMethod Previous { $this.Slide.SlideIndex--; $this.CurrentShowPosition-- }
$view | Add-Member ScriptMethod First { $this.Slide.SlideIndex = 1; $this.CurrentShowPosition = 1 }
$view | Add-Member ScriptMethod Last { $this.Slide.SlideIndex = 3; $this.CurrentShowPosition = 3 }
$view | Add-Member ScriptMethod GotoSlide { param($index) $this.Slide.SlideIndex = $index; $this.CurrentShowPosition = $index }
$window = [pscustomobject]@{ View = $view }
$slides = [pscustomobject]@{ Count = 3 }
$slides | Add-Member ScriptMethod Item { param($index) return [pscustomobject]@{ SlideShowTransition = [pscustomobject]@{ Hidden = 0 } } }
$deck = [pscustomobject]@{ FullName = $path; Slides = $slides; SlideShowWindow = $window }

Assert-Equal (Move-Show $deck $window 'next') "step`t3`t3`trunning" 'last-slide animation'
Assert-Equal $view.Advances 1 'animation advanced once'
Assert-Equal (Move-Show $deck $window 'next') "step`t3`t3`tdone" 'black end screen'
Assert-Equal $view.State 3 'end screen stays captured'
$null = Move-Show $deck $window 'next'
Assert-Equal $view.Advances 1 'next after end does not close show'
Assert-Equal (Move-Show $deck $window 'previous') "step`t3`t3`trunning" 'back from end restores final slide'
Assert-Equal (Move-Show $deck $window ([pscustomobject]@{ slide = 99 })) "step`t3`t3`trunning" 'goto clamps high'
Assert-Equal (Move-Show $deck $window ([pscustomobject]@{ slide = 0 })) "step`t1`t1`trunning" 'goto clamps low'
Assert-Equal (Move-Show $deck $window 'last') "step`t3`t3`trunning" 'last'
Assert-Equal (Move-Show $deck $window 'first') "step`t1`t1`trunning" 'first'

# A first/goto command must leave the emulated end screen.
$view.Slide.SlideIndex = 3
$view.CurrentShowPosition = 3
$view.Click = $view.Clicks
$null = Move-Show $deck $window 'next'
Assert-Equal (Move-Show $deck $window 'first') (('step', 1, 1, 'running') -join [char]9) 'first restores slide after end'
Assert-Equal $view.State 1 'first clears the black end state'

# Verify that Player is addressed by shape ID rather than collection index.
$player = [pscustomobject]@{ State = 0; CurrentPosition = 1250 }
$player | Add-Member ScriptMethod Play { $this.State = 0 }
$player | Add-Member ScriptMethod Pause { $this.State = 1 }
$shapes = [pscustomobject]@{ Count = 1 }
$shapes | Add-Member ScriptMethod Item { param($index) return [pscustomobject]@{ Type = 16; Id = 42 } }
$slide | Add-Member NoteProperty Shapes $shapes
$view | Add-Member NoteProperty Media $player
$view | Add-Member ScriptMethod Player { param($id) if ($id -ne 42) { throw 'Incorrect shape ID' }; return $this.Media }
$request = [pscustomobject]@{ known = $null; control = 'pause'; position = 0 }
Assert-Equal (Get-Video $deck $window $request) "video`t1`t1`tpaused`t1250" 'video pause'
$request.control = 'seek'
$request.position = 8000
Assert-Equal (Get-Video $deck $window $request) "video`t1`t1`tpaused`t8000" 'video seek'
$request.control = 'restart'
Assert-Equal (Get-Video $deck $window $request) "video`t1`t1`tplaying`t0" 'video restart'
Write-Host 'Windows PowerPoint command tests passed.'

# Upstream's show-count and PDF requests use native COM, not AppleScript.
$app | Add-Member NoteProperty SlideShowWindows ([pscustomobject]@{ Count = 0 })
$app.Presentations = @($deck)
$script:powerpoint = $app
Assert-Equal (Invoke-PowerPointCommand ([pscustomobject]@{ op = 'slide_shows' })) "shows`t0" 'show count without a deck path'
$deck | Add-Member NoteProperty Exported $null
$deck | Add-Member ScriptMethod ExportAsFixedFormat {
    param($out, $kind, $intent, $frame, $order, $output, $hidden, $range, $rangeType)
    $this.Exported = @($out, $kind, $output, $hidden, $range, $rangeType)
}
$pdfRequest = [pscustomobject]@{ op = 'save_pdf'; path = $path; out = '\\?\C:\Talks\slides.pdf' }
Assert-Equal (Invoke-PowerPointCommand $pdfRequest) 'saved' 'PDF copy exported'
Assert-Equal $deck.Exported[0] 'C:\Talks\slides.pdf' 'PDF output path normalized'
Assert-Equal $deck.Exported[1] 2 'PDF fixed format'
Assert-Equal $deck.Exported[2] 1 'slides, not handouts'
Assert-Equal $deck.Exported[3] -1 'hidden slides included'
Assert-Equal $deck.Exported[4] $null 'all slides print range'
Assert-Equal $deck.Exported[5] 1 'all slides range type'
Assert-Equal $deck.FullName $path 'original deck unchanged'
$app.SlideShowWindows.Count = 1
Assert-Equal (Invoke-PowerPointCommand ([pscustomobject]@{ op = 'slide_shows' })) "shows`t1" 'running show counted'
$deck.Exported = $null
Assert-Equal (Invoke-PowerPointCommand $pdfRequest) 'showing' 'running show blocks PDF export'
Assert-Equal $deck.Exported $null 'no export during a show'
Write-Host 'Windows upstream PowerPoint command tests passed.'
