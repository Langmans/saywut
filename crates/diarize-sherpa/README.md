# saywut-diarize-sherpa

Speaker diarization and voice embeddings via sherpa-onnx.

- **Diarization** (`SherpaDiarizer`): pyannote segmentation finds speech
  turns, a speaker-embedding model (CAM++) turns them into voice vectors, and
  clustering groups the vectors into speakers, by threshold or by a given
  number of speakers.
- **Voice embeddings** (`SherpaEmbedder`): the same embedding model on its
  own, for voice profiles.

Both model files come from the manifest (`saywut-models`); this crate only
loads the paths it is given. Both models need 16 kHz mono.

Building: the sherpa-onnx libraries are downloaded at build time, and their
build script needs `target/<profile>/deps` to exist (see the repository
README).
