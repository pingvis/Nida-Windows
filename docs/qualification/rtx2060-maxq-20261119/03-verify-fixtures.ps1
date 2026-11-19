Get-Command ffmpeg, ffprobe -ErrorAction Stop | Out-Null
$source=Join-Path $PWD 'target/gpu-qualification-repro/fixtures'
$verified=Join-Path $PWD 'target/gpu-qualification-repro/fixtures-verified'
New-Item -ItemType Directory -Force $verified|Out-Null
foreach($clip in Get-ChildItem $source -Filter '*.mp4') {
 $probe=& ffprobe -v error -show_streams -of json $clip.FullName|ConvertFrom-Json
 $codec=$probe.streams[0].codec_name
 $prim=if($clip.BaseName -in @('hdr10-1080p60','hlg-1080p60')){9}elseif($clip.BaseName -eq 'bt601'){6}else{1}
 $trc=if($clip.BaseName -eq 'hdr10-1080p60'){16}elseif($clip.BaseName -eq 'hlg-1080p60'){18}elseif($clip.BaseName -eq 'bt601'){6}else{1}
 $matrix=if($prim -eq 9){9}elseif($prim -eq 6){6}else{1}
 $range=if($clip.BaseName -eq 'full-1080p60'){1}else{0}
 $filter=if($codec -eq 'av1'){"av1_metadata=color_primaries=${prim}:transfer_characteristics=${trc}:matrix_coefficients=${matrix}:color_range=$range"}else{"${codec}_metadata=colour_primaries=${prim}:transfer_characteristics=${trc}:matrix_coefficients=${matrix}:video_full_range_flag=$range"}
 $ffargs=@('-hide_banner','-loglevel','error','-nostdin','-n')
 if($clip.BaseName -eq 'rotation90'){$ffargs+=@('-display_rotation:v:0','90')}
 $ffargs+=@('-i',$clip.FullName,'-map','0:v:0','-c:v','copy')
 if($clip.BaseName -ne 'untagged'){$ffargs+=@('-bsf:v',$filter,'-color_primaries',"$prim",'-color_trc',"$trc",'-colorspace',"$matrix",'-color_range',$(if($range){'pc'}else{'tv'}))}
 $output=Join-Path $verified $clip.Name
 $ffargs+=$output
 $ffargs|ConvertTo-Json -Compress|Add-Content "$verified/commands.jsonl"
 & ffmpeg @ffargs
 if($LASTEXITCODE -ne 0){throw "Retag failed $($clip.Name)"}
 & ffprobe -v error -show_streams -show_format -of json $output|Set-Content "$output.probe.json"
 $actual=(Get-Content "$output.probe.json" -Raw|ConvertFrom-Json).streams[0]
 if($clip.BaseName -ne 'untagged' -and (!$actual.color_primaries -or !$actual.color_transfer)){throw "Missing colour tags $($clip.Name)"}
 if($clip.BaseName -eq 'hdr10-1080p60' -and $actual.color_transfer -ne 'smpte2084'){throw 'PQ tag missing'}
 if($clip.BaseName -eq 'hlg-1080p60' -and $actual.color_transfer -ne 'arib-std-b67'){throw 'HLG tag missing'}
 if($clip.BaseName -eq 'rotation90' -and !($actual.side_data_list|Where-Object {$_.rotation -eq 90})){throw '90-degree rotation missing'}
 Get-FileHash $output -Algorithm SHA256|Select-Object Path,Hash|ConvertTo-Json -Compress|Add-Content "$verified/hashes.jsonl"
}
