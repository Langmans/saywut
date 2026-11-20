### Saywut UI strings, English. Every locale has exactly the same keys.

# Label for a speaker the user has not named yet. $n starts at 1.
speaker-label = Speaker { $n }

error-read-audio = Could not read audio file { $path }: { $reason }
error-read-script = Could not read fake script { $path }: { $reason }
error-write-output = Could not write { $path }: { $reason }
error-unsupported-language = Language { $language } is not supported by this engine.
error-asr-engine = Speech recognition failed: { $reason }

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
