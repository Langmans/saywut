<#
.SYNOPSIS
Generates synthetic speech clips with the Windows voices, plus a reference
transcript for each, for pipeline and diarization tests.

.DESCRIPTION
Every line is synthesized on its own as 16 kHz mono 16-bit PCM and the lines are
joined with a fixed silence in between. Start and end times are therefore known
to the sample, and each clip gets a `<name>.json` in Saywut's transcript format
(version 1) next to its `<name>.wav`.

Synthetic speech is clean and evenly paced: good for checking that the pipeline
and the diarizer work, not for measuring accuracy on real recordings.

A clip whose voice is not installed is skipped with a warning. Voices are added
under Settings > Time & language > Speech.

.PARAMETER OutDir
Where the clips are written. Defaults to testdata/tts in the repository.
#>
param(
    [string]$OutDir = (Join-Path $PSScriptRoot '..\testdata\tts')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Speech

$SampleRate = 16000
$GapMs = 600

# Each clip: language tag, speakers (index = speaker id) and lines as
# speaker index + text.
$Clips = [ordered]@{
    'en-dialogue' = @{
        Language = 'en'
        Voices   = @('Microsoft David Desktop', 'Microsoft Zira Desktop')
        Lines    = @(
            @(0, 'Good morning. Did you manage to finish the report yesterday?'),
            @(1, 'Almost. I still need the numbers from the second quarter.'),
            @(0, 'I can send those after lunch. Is that soon enough?'),
            @(1, 'That works. Then I will have it ready by Friday.'),
            @(0, 'Great. Let us go over it together on Monday.'),
            @(1, 'Fine by me. I will book a room for ten o''clock.')
        )
    }
    'en-three-speakers' = @{
        Language = 'en'
        Voices   = @('Microsoft David Desktop', 'Microsoft Zira Desktop', 'Microsoft Mark')
        Lines    = @(
            @(0, 'Welcome everyone. Shall we start with the planning?'),
            @(1, 'Yes. The new version is nearly done.'),
            @(2, 'The tests are still failing on macOS, though.'),
            @(0, 'How long do you think that will take?'),
            @(2, 'Two days, if nothing else turns up.'),
            @(1, 'Then we release next week.')
        )
    }
    'nl-monologue' = @{
        Language = 'nl'
        Voices   = @('Microsoft Frank')
        Lines    = @(
            @(0, 'Goedemorgen, en welkom bij deze korte test.'),
            @(0, 'Vandaag kijken we of de spraakherkenning Nederlands goed verstaat.'),
            @(0, 'Het weer is wisselend, met af en toe een bui.'),
            @(0, 'Morgen wordt het droger, met een temperatuur van twaalf graden.'),
            @(0, 'Dat was het voor nu. Tot de volgende keer.')
        )
    }
}

function New-WavBytes([byte[]]$Pcm) {
    $ms = New-Object System.IO.MemoryStream
    $w = New-Object System.IO.BinaryWriter($ms)
    $w.Write([Text.Encoding]::ASCII.GetBytes('RIFF'))
    $w.Write([int](36 + $Pcm.Length))
    $w.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt '))
    $w.Write([int]16)               # fmt chunk size
    $w.Write([int16]1)              # PCM
    $w.Write([int16]1)              # mono
    $w.Write([int]$SampleRate)
    $w.Write([int]($SampleRate * 2)) # byte rate
    $w.Write([int16]2)              # block align
    $w.Write([int16]16)             # bits per sample
    $w.Write([Text.Encoding]::ASCII.GetBytes('data'))
    $w.Write([int]$Pcm.Length)
    $w.Write($Pcm)
    $w.Flush()
    $ms.ToArray()
}

$format = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(
    $SampleRate, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,
    [System.Speech.AudioFormat.AudioChannel]::Mono)
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
$installed = $synth.GetInstalledVoices() | ForEach-Object { $_.VoiceInfo.Name }
$gap = New-Object byte[] ($SampleRate * 2 * $GapMs / 1000)

New-Item -ItemType Directory -Force $OutDir | Out-Null

foreach ($name in $Clips.Keys) {
    $clip = $Clips[$name]
    $missing = $clip.Voices | Where-Object { $_ -notin $installed }
    if ($missing) {
        Write-Warning "$name skipped, voice not installed: $($missing -join ', ')"
        continue
    }

    $pcm = New-Object System.IO.MemoryStream
    $segments = @()
    foreach ($line in $clip.Lines) {
        $speaker, $text = $line
        $synth.SelectVoice($clip.Voices[$speaker])
        $buf = New-Object System.IO.MemoryStream
        $synth.SetOutputToAudioStream($buf, $format)
        $synth.Speak($text)
        $synth.SetOutputToNull()

        $startMs = [long]($pcm.Length / 2 * 1000 / $SampleRate)
        $bytes = $buf.ToArray()
        $pcm.Write($bytes, 0, $bytes.Length)
        $endMs = [long]($pcm.Length / 2 * 1000 / $SampleRate)
        $pcm.Write($gap, 0, $gap.Length)

        $segments += [ordered]@{ start_ms = $startMs; end_ms = $endMs; speaker = $speaker; text = $text }
    }

    $speakers = for ($i = 0; $i -lt $clip.Voices.Count; $i++) {
        [ordered]@{ id = $i; name = ($clip.Voices[$i] -replace '^Microsoft | Desktop$', '') }
    }
    $transcript = [ordered]@{
        version  = 1
        language = $clip.Language
        speakers = @($speakers)
        segments = $segments
    }

    [IO.File]::WriteAllBytes((Join-Path $OutDir "$name.wav"), (New-WavBytes $pcm.ToArray()))
    $transcript | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8NoBOM (Join-Path $OutDir "$name.json")
    $seconds = [math]::Round($pcm.Length / 2 / $SampleRate, 1)
    Write-Host "$name.wav  ${seconds}s, $($segments.Count) lines, $($clip.Voices.Count) speaker(s)"
}

$synth.Dispose()
