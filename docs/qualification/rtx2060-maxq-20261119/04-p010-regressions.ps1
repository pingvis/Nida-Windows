Get-Command ffmpeg, ffprobe -ErrorAction Stop | Out-Null
$directory=Join-Path $PWD 'target/gpu-qualification-repro/p010-regression-fixtures'
New-Item -ItemType Directory -Force $directory|Out-Null
foreach($case in @(@{Name='p010-full';Filter='scale=in_range=tv:out_range=pc,setparams=range=full:color_primaries=bt709:color_trc=bt709:colorspace=bt709';Range='pc'},@{Name='p010-wide';Filter='setsar=4/3,setparams=range=limited:color_primaries=bt709:color_trc=bt709:colorspace=bt709';Range='tv'},@{Name='p010-narrow';Filter='setsar=3/4,setparams=range=limited:color_primaries=bt709:color_trc=bt709:colorspace=bt709';Range='tv'})){
 $clip=Join-Path $directory "$($case.Name).mp4"
 $ffargs=@('-hide_banner','-loglevel','error','-nostdin','-n','-f','lavfi','-i','testsrc2=size=1920x1080:rate=60','-t','4','-an','-vf',$case.Filter,'-c:v','libx265','-preset','veryfast','-x265-params','pools=4:frame-threads=2','-threads','4','-pix_fmt','yuv420p10le','-tag:v','hvc1','-g','120','-colorspace','bt709','-color_primaries','bt709','-color_trc','bt709','-color_range',$case.Range,$clip)
 $ffargs|ConvertTo-Json -Compress|Add-Content "$directory/commands.jsonl"
 & ffmpeg @ffargs
 if($LASTEXITCODE -ne 0){throw "P010 fixture failed $($case.Name)"}
 & ffprobe -v error -show_streams -show_format -of json $clip|Set-Content "$clip.probe.json"
 Get-FileHash $clip|Select-Object Path,Hash|ConvertTo-Json -Compress|Add-Content "$directory/hashes.jsonl"
}
