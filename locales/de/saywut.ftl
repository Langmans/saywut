### Saywut UI strings, German. Every locale has exactly the same keys.

# Label for a speaker the user has not named yet. $n starts at 1.
speaker-label = Sprecher { $n }

error-read-audio = Audiodatei { $path } konnte nicht gelesen werden: { $reason }
error-read-script = Testskript { $path } konnte nicht gelesen werden: { $reason }
error-write-output = { $path } konnte nicht geschrieben werden: { $reason }
error-unsupported-language = Die Sprache { $language } wird von dieser Engine nicht unterstützt.
error-asr-engine = Spracherkennung fehlgeschlagen: { $reason }

error-diarize = Sprechererkennung fehlgeschlagen: { $reason }
error-read-transcript = Transkript { $path } konnte nicht gelesen werden: { $reason }
error-speaker-unknown = In diesem Transkript gibt es keinen Sprecher { $n }.
error-speaker-same = Sprecher { $n } kann nicht mit sich selbst zusammengeführt werden.

# One line of `saywut speakers list`: number, name or label, number of lines.
speakers-list-line = { $n }  { $label }  { $lines ->
    [one] 1 Zeile
   *[other] { $lines } Zeilen
}

# A name heard for speaker $n; confirm with `saywut speakers rename`.
speakers-suggest-line = { $n }  { $name }  ← "{ $text }"
speakers-suggest-none = Keine Namen gehört.

error-read-profiles = Stimmprofile { $path } konnten nicht gelesen werden: { $reason }
error-speaker-silent = Sprecher { $n } sagt in diesem Transkript nichts.
error-profile-unknown = Es gibt kein Stimmprofil für { $name }.
profiles-none = Keine Stimmprofile gespeichert.
profiles-list-line = { $name }  ({ $recordings ->
    [one] 1 Aufnahme
   *[other] { $recordings } Aufnahmen
})
profiles-added = Stimme von { $name } gespeichert.
profiles-deleted = Stimmprofil von { $name } gelöscht.

error-model-unknown = Unbekanntes Modell { $id }. Siehe: saywut models list
error-model-missing = Modell { $id } ist noch nicht heruntergeladen. Ausführen: saywut models fetch { $id }
error-model-corrupt = Modelldatei { $path } ist beschädigt ({ $reason }). Ausführen: saywut models fetch { $id }
error-model-fetch = Modell { $id } konnte nicht geladen werden: { $reason }

models-ready = Modell { $id } ist bereit: { $path }
# One line of `saywut models list`; $size is in MB.
models-list-line = { $id }  { $engine }  { $size } MB  { $languages }  { $status }
models-languages-any = jede Sprache
models-status-present = heruntergeladen
models-status-missing = nicht heruntergeladen
