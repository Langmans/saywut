### Saywut UI strings, Dutch. Every locale has exactly the same keys.

# Label for a speaker the user has not named yet. $n starts at 1.
speaker-label = Spreker { $n }

## Desktop app. Every app-* message goes to the frontend unformatted, so
## these take no arguments.

app-title = Saywut
app-always-on-top = Bovenop
app-start = Start
app-stop = Stop
app-status-live = Live
app-status-stopped = Gestopt
app-transcript-empty = Nog niets gezegd.
# Accessible name of a source's level meter.
app-level = Niveau
app-source-microphone = Microfoon
app-source-loopback = Systeemgeluid
app-source-application = App
app-source-file = Bestand

error-read-audio = Kon audiobestand { $path } niet lezen: { $reason }
error-read-script = Kon nepscript { $path } niet lezen: { $reason }
error-write-output = Kon { $path } niet schrijven: { $reason }
error-unsupported-language = Taal { $language } wordt niet ondersteund door deze engine.
error-asr-engine = Spraakherkenning mislukt: { $reason }

error-diarize = Sprekerherkenning mislukt: { $reason }
error-read-transcript = Kon transcript { $path } niet lezen: { $reason }
error-speaker-unknown = Er is geen spreker { $n } in dit transcript.
error-speaker-same = Spreker { $n } kan niet met zichzelf worden samengevoegd.

# One line of `saywut speakers list`: number, name or label, number of lines.
speakers-list-line = { $n }  { $label }  { $lines ->
    [one] 1 regel
   *[other] { $lines } regels
}

# A name heard for speaker $n; confirm with `saywut speakers rename`.
speakers-suggest-line = { $n }  { $name }  ← "{ $text }"
speakers-suggest-none = Geen namen gehoord.

error-read-profiles = Kon stemprofielen { $path } niet lezen: { $reason }
error-speaker-silent = Spreker { $n } zegt niets in dit transcript.
error-profile-unknown = Er is geen stemprofiel van { $name }.
profiles-none = Geen stemprofielen opgeslagen.
profiles-list-line = { $name }  ({ $recordings ->
    [one] 1 opname
   *[other] { $recordings } opnames
})
profiles-added = Stem van { $name } opgeslagen.
profiles-deleted = Stemprofiel van { $name } verwijderd.

error-model-unknown = Onbekend model { $id }. Zie: saywut models list
error-model-missing = Model { $id } is nog niet gedownload. Voer uit: saywut models fetch { $id }
error-model-corrupt = Modelbestand { $path } is beschadigd ({ $reason }). Voer uit: saywut models fetch { $id }
error-model-fetch = Kon model { $id } niet ophalen: { $reason }

models-ready = Model { $id } staat klaar: { $path }
# One line of `saywut models list`; $size is in MB.
models-list-line = { $id }  { $engine }  { $size } MB  { $languages }  { $status }
models-languages-any = elke taal
models-status-present = gedownload
models-status-missing = niet gedownload
