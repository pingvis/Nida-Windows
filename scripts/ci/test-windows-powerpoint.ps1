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
$path = "C:\Talks\$unicode ' dollar\$().pptx"
$first = [pscustomobject]@{ FullName = $path }
$second = [pscustomobject]@{ FullName = 'C:\Other\talk.pptx' }
$app = [pscustomobject]@{ Presentations = @($first, $second) }
Assert-Equal (Find-Deck $app $path.ToUpperInvariant()).FullName $path 'case-insensitive full path'
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
