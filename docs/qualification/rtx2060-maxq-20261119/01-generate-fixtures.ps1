Get-Command ffmpeg, ffprobe -ErrorAction Stop | Out-Null
$fixtureDir = Join-Path $PWD 'target/gpu-qualification-repro/fixtures'
New-Item -ItemType Directory -Force $fixtureDir | Out-Null
$cases = @(
 @{Name='h264-1080p60';Size='1920x1080';Seconds=120;Codec='libx264';Pixel='yuv420p'},
 @{Name='h264-4k60';Size='3840x2160';Seconds=120;Codec='libx264';Pixel='yuv420p'},
 @{Name='hevc-1080p60';Size='1920x1080';Seconds=4;Codec='libx265';Pixel='yuv420p'},
 @{Name='hevc10-1080p60';Size='1920x1080';Seconds=4;Codec='libx265';Pixel='yuv420p10le'},
 @{Name='hdr10-1080p60';Size='1920x1080';Seconds=4;Codec='libx265';Pixel='yuv420p10le';Trc='smpte2084'},
 @{Name='hlg-1080p60';Size='1920x1080';Seconds=4;Codec='libx265';Pixel='yuv420p10le';Trc='arib-std-b67'},
 @{Name='av1-1080p60';Size='1920x1080';Seconds=4;Codec='libsvtav1';Pixel='yuv420p10le'},
 @{Name='full-1080p60';Size='1920x1080';Seconds=4;Codec='libx264';Pixel='yuv420p';Range='pc'},
 @{Name='bt601';Size='720x480';Seconds=4;Codec='libx264';Pixel='yuv420p';Matrix='smpte170m'},
 @{Name='wide-sar';Size='1920x1080';Seconds=4;Codec='libx264';Pixel='yuv420p';Filter='setsar=4/3'},
 @{Name='narrow-sar';Size='1920x1080';Seconds=4;Codec='libx264';Pixel='yuv420p';Filter='setsar=3/4'},
 @{Name='padded';Size='1918x1078';Seconds=4;Codec='libx264';Pixel='yuv420p'},
 @{Name='interlaced';Size='1920x1080';Seconds=4;Codec='libx264';Pixel='yuv420p';Filter='tinterlace=interleave_top'}
)
foreach ($case in $cases) {
 $range=if($case.Range){$case.Range}else{'tv'}
 $matrix=if($case.Matrix){$case.Matrix}elseif($case.Trc){'bt2020nc'}else{'bt709'}
 $prim=if($case.Trc){'bt2020'}elseif($case.Matrix){'smpte170m'}else{'bt709'}
 $trc=if($case.Trc){$case.Trc}else{'bt709'}
 $preset=if($case.Codec -eq 'libsvtav1'){'12'}else{'veryfast'}
 $args=@('-hide_banner','-loglevel','error','-nostdin','-n','-f','lavfi','-i',"testsrc2=size=$($case.Size):rate=60",'-t',"$($case.Seconds)",'-an')
 if($case.Filter){$args+=@('-vf',$case.Filter)}
 $args+=@('-c:v',$case.Codec,'-preset',$preset,'-threads','4','-pix_fmt',$case.Pixel,'-g','120','-colorspace',$matrix,'-color_primaries',$prim,'-color_trc',$trc,'-color_range',$range)
 if($case.Codec -eq 'libx265'){$args+=@('-x265-params','pools=4:frame-threads=2','-tag:v','hvc1')}
 if($case.Codec -eq 'libsvtav1'){$args+=@('-svtav1-params','lp=4')}
 if($case.Name -eq 'interlaced'){$args+=@('-flags','+ilme+ildct','-x264-params','tff=1')}
 $output=Join-Path $fixtureDir "$($case.Name).mp4"
 $args+=$output
 ($args | ConvertTo-Json -Compress) | Add-Content "$fixtureDir/commands.jsonl"
 & ffmpeg @args
 if($LASTEXITCODE -ne 0){throw "Fixture failed: $($case.Name)"}
 & ffprobe -v error -show_streams -show_format -of json $output | Set-Content "$output.probe.json"
 Get-FileHash $output -Algorithm SHA256 | Select-Object Path,Hash | ConvertTo-Json -Compress | Add-Content "$fixtureDir/hashes.jsonl"
}
& ffmpeg -hide_banner -loglevel error -nostdin -n -i "$fixtureDir/hevc-1080p60.mp4" -c copy -metadata:s:v:0 rotate=90 "$fixtureDir/rotation90.mp4"
if($LASTEXITCODE -ne 0){throw 'Rotation fixture failed'}
Get-FileHash "$fixtureDir/rotation90.mp4" -Algorithm SHA256 | Select-Object Path,Hash | ConvertTo-Json -Compress | Add-Content "$fixtureDir/hashes.jsonl"
& ffprobe -v error -show_streams -show_format -of json "$fixtureDir/rotation90.mp4" | Set-Content "$fixtureDir/rotation90.mp4.probe.json"
