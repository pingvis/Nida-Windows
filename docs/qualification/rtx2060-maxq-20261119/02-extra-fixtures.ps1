Get-Command ffmpeg, ffprobe -ErrorAction Stop | Out-Null
$fixtureDir=Join-Path $PWD 'target/gpu-qualification-repro/fixtures'
& ffmpeg -hide_banner -loglevel error -nostdin -n -i "$fixtureDir/h264-1080p60.mp4" -t 4 -c copy "$fixtureDir/h264-playlist.mp4"
if($LASTEXITCODE -ne 0){throw 'Playlist fixture failed'}
& ffmpeg -hide_banner -loglevel error -nostdin -n -f lavfi -i 'testsrc2=size=3840x2160:rate=30' -t 4 -an -c:v libx264 -threads 4 -preset veryfast -pix_fmt yuv420p -colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv "$fixtureDir/h264-4k30.mp4"
if($LASTEXITCODE -ne 0){throw '4k30 fixture failed'}
& ffmpeg -hide_banner -loglevel error -nostdin -n -f lavfi -i 'testsrc2=size=1920x1080:rate=60' -t 4 -an -c:v libx264 -threads 4 -preset veryfast -pix_fmt yuv420p "$fixtureDir/untagged.mp4"
if($LASTEXITCODE -ne 0){throw 'Untagged fixture failed'}
foreach($name in @('h264-playlist','h264-4k30','untagged')) {
 $clip=Join-Path $fixtureDir "$name.mp4"
 Get-FileHash $clip -Algorithm SHA256|Select-Object Path,Hash|ConvertTo-Json -Compress|Add-Content "$fixtureDir/hashes.jsonl"
 & ffprobe -v error -show_streams -show_format -of json $clip|Set-Content "$clip.probe.json"
}
