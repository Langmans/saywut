### Saywut UI strings, English. Every locale has exactly the same keys.

# Label for a speaker the user has not named yet. $n starts at 1.
speaker-label = Speaker { $n }

error-read-audio = Could not read audio file { $path }: { $reason }
error-read-script = Could not read fake script { $path }: { $reason }
error-write-output = Could not write { $path }: { $reason }
error-unsupported-language = Language { $language } is not supported by this engine.
error-asr-engine = Speech recognition failed: { $reason }

error-diarize = Diarization failed: { $reason }
error-read-transcript = Could not read transcript { $path }: { $reason }
error-speaker-unknown = There is no speaker { $n } in this transcript.
error-speaker-same = Speaker { $n } cannot be merged into itself.

# One line of `saywut speakers list`: number, name or label, number of lines.
speakers-list-line = { $n }  { $label }  { $lines ->
    [one] 1 line
   *[other] { $lines } lines
}

# A name heard for speaker $n; confirm with `saywut speakers rename`.
speakers-suggest-line = { $n }  { $name }  ← "{ $text }"
speakers-suggest-none = No names heard.

error-read-profiles = Could not read voice profiles { $path }: { $reason }
error-speaker-silent = Speaker { $n } has no speech in this transcript.
error-profile-unknown = There is no voice profile for { $name }.
profiles-none = No voice profiles stored.
profiles-list-line = { $name }  ({ $recordings ->
    [one] 1 recording
   *[other] { $recordings } recordings
})
profiles-added = Voice of { $name } stored.
profiles-deleted = Voice profile of { $name } deleted.

error-model-unknown = Unknown model { $id }. See: saywut models list
error-model-missing = Model { $id } is not downloaded yet. Run: saywut models fetch { $id }
error-model-corrupt = Model file { $path } is damaged ({ $reason }). Run: saywut models fetch { $id }
error-model-fetch = Could not fetch model { $id }: { $reason }

models-ready = Model { $id } is ready: { $path }
# One line of `saywut models list`; $size is in MB.
models-list-line = { $id }  { $engine }  { $size } MB  { $languages }  { $status }
models-languages-any = any language
models-status-present = downloaded
models-status-missing = not downloaded
