//! The model manifest (`models.toml`, embedded at build time), where model
//! files live on disk, and verified downloads. See "Models" in PLAN.md.

use std::{
    fmt, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The repository's `models.toml`, compiled in so the binary and the manifest
/// can never disagree.
const BUILTIN: &str = include_str!("../../../models.toml");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub id: String,
    pub engine: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub license: String,
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub models: Vec<Model>,
}

#[derive(Debug)]
pub enum ModelError {
    Manifest(String),
    UnknownModel(String),
    Io(io::Error),
    Download(String),
    SizeMismatch { expected: u64, actual: u64 },
    HashMismatch { expected: String, actual: String },
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(msg) => write!(f, "invalid model manifest: {msg}"),
            Self::UnknownModel(id) => write!(f, "unknown model: {id}"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Download(msg) => write!(f, "download failed: {msg}"),
            Self::SizeMismatch { expected, actual } => {
                write!(f, "size is {actual} bytes, expected {expected}")
            }
            Self::HashMismatch { expected, actual } => {
                write!(f, "sha256 is {actual}, expected {expected}")
            }
        }
    }
}

impl std::error::Error for ModelError {}

impl From<io::Error> for ModelError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Lowercase letters, digits and `-`: safe as a path component everywhere.
fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl Manifest {
    pub fn builtin() -> Self {
        Self::parse(BUILTIN).expect("models.toml is valid")
    }

    /// Parses and validates a manifest. `id` and `engine` become path
    /// components and the URL is fetched, so all three are checked here.
    pub fn parse(source: &str) -> Result<Self, ModelError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct File {
            model: Vec<Model>,
        }
        let file: File = toml::from_str(source).map_err(|e| ModelError::Manifest(e.to_string()))?;
        for (i, m) in file.model.iter().enumerate() {
            let bad = |what: &str| ModelError::Manifest(format!("model {i} ({}): {what}", m.id));
            if !is_slug(&m.id) || !is_slug(&m.engine) {
                return Err(bad(
                    "id and engine must be lowercase letters, digits and '-'",
                ));
            }
            if !m.url.starts_with("https://") {
                return Err(bad("url must use https"));
            }
            if m.extension().is_none() {
                return Err(bad("url must end in a file name with an extension"));
            }
            if m.sha256.len() != 64
                || !m
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err(bad("sha256 must be 64 lowercase hex digits"));
            }
            if file.model[..i].iter().any(|o| o.id == m.id) {
                return Err(bad("duplicate id"));
            }
        }
        Ok(Self { models: file.model })
    }

    pub fn get(&self, id: &str) -> Result<&Model, ModelError> {
        self.models
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| ModelError::UnknownModel(id.to_owned()))
    }
}

impl Model {
    /// Extension of the file the URL points at, if it is a plain one.
    fn extension(&self) -> Option<&str> {
        let name = self.url.rsplit('/').next()?;
        let (_, ext) = name.rsplit_once('.')?;
        (!ext.is_empty() && ext.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(ext)
    }

    /// `<dir>/<engine>/<id>-<first 12 of sha256>.<ext>`: versions of one
    /// model sit side by side, and a file's name says which version it is.
    pub fn path(&self, dir: &Path) -> PathBuf {
        let ext = self.extension().expect("validated by Manifest::parse");
        dir.join(&self.engine)
            .join(format!("{}-{}.{ext}", self.id, &self.sha256[..12]))
    }
}

/// Streams `reader` into `writer`, at most `limit` bytes, returning the byte
/// count and sha256. One byte past `limit` is read so an oversized source is
/// detected without reading it all.
fn copy_hashed(
    reader: impl Read,
    writer: &mut impl Write,
    limit: u64,
) -> io::Result<(u64, String)> {
    let mut reader = reader.take(limit + 1);
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 16];
    let mut total = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        writer.write_all(&buf[..n])?;
        total += n as u64;
    }
    let hex = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((total, hex))
}

fn check(model: &Model, size: u64, sha256: String) -> Result<(), ModelError> {
    if size != model.size {
        return Err(ModelError::SizeMismatch {
            expected: model.size,
            actual: size,
        });
    }
    if sha256 != model.sha256 {
        return Err(ModelError::HashMismatch {
            expected: model.sha256.clone(),
            actual: sha256,
        });
    }
    Ok(())
}

/// Checks the file at `path` against the manifest entry.
pub fn verify(model: &Model, path: &Path) -> Result<(), ModelError> {
    let (size, sha256) = copy_hashed(fs::File::open(path)?, &mut io::sink(), model.size)?;
    check(model, size, sha256)
}

/// Opens `url` for reading; [`http_get`] in production, a fake in tests.
pub type Opener<'a> = &'a dyn Fn(&str) -> Result<Box<dyn Read>, ModelError>;

pub fn http_get(url: &str) -> Result<Box<dyn Read>, ModelError> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| ModelError::Download(e.to_string()))?;
    Ok(Box::new(response.into_body().into_reader()))
}

/// Makes sure the model is on disk and intact, downloading it if not.
/// Downloads go to `<file>.part` and are renamed only after size and sha256
/// match, so a half or wrong download never looks like a model.
pub fn fetch(model: &Model, dir: &Path, open: Opener) -> Result<PathBuf, ModelError> {
    let path = model.path(dir);
    if verify(model, &path).is_ok() {
        return Ok(path);
    }
    fs::create_dir_all(path.parent().expect("path has an engine folder"))?;
    let part = path.with_file_name(format!(
        "{}.part",
        path.file_name()
            .expect("path has a file name")
            .to_string_lossy()
    ));
    let result = (|| {
        let mut file = fs::File::create(&part)?;
        let (size, sha256) = copy_hashed(open(&model.url)?, &mut file, model.size)?;
        file.sync_all()?;
        drop(file);
        check(model, size, sha256)?;
        fs::rename(&part, &path)?;
        Ok(path.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, net::TcpListener, thread};

    const DATA: &[u8] = b"model bytes";
    /// sha256 of DATA.
    fn data_sha() -> String {
        copy_hashed(DATA, &mut io::sink(), 100).unwrap().1
    }

    fn model() -> Model {
        Model {
            id: "test-model".into(),
            engine: "fake".into(),
            url: "https://example.invalid/files/test.bin".into(),
            sha256: data_sha(),
            size: DATA.len() as u64,
            license: "MIT".into(),
            languages: vec!["en".into()],
        }
    }

    fn toml_entry(model: &Model) -> String {
        format!(
            "[[model]]\nid = {:?}\nengine = {:?}\nurl = {:?}\nsha256 = {:?}\nsize = {}\nlicense = \"MIT\"\nlanguages = [\"en\"]\n",
            model.id, model.engine, model.url, model.sha256, model.size
        )
    }

    struct Scratch(PathBuf);
    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("saywut-models-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn serve(data: &'static [u8]) -> Box<dyn Read> {
        Box::new(Cursor::new(data))
    }

    /// An opener serving the correct model bytes.
    fn serve_data(_url: &str) -> Result<Box<dyn Read>, ModelError> {
        Ok(serve(DATA))
    }

    #[test]
    fn builtin_manifest_is_valid_and_has_whisper_base() {
        let m = Manifest::builtin();
        let base = m.get("whisper-base").unwrap();
        assert_eq!(base.engine, "whisper");
        assert!(base.url.contains("/resolve/") && !base.url.contains("/main/"));
    }

    #[test]
    fn parse_round_trip_and_lookup() {
        let m = Manifest::parse(&toml_entry(&model())).unwrap();
        assert_eq!(m.models, vec![model()]);
        let err = m.get("nope").unwrap_err();
        assert!(matches!(&err, ModelError::UnknownModel(id) if id == "nope"));
        assert_eq!(err.to_string(), "unknown model: nope");
    }

    #[test]
    fn parse_rejects_bad_entries() {
        type Mutation = Box<dyn Fn(&mut Model)>;
        let cases: Vec<(Mutation, &str)> = vec![
            (Box::new(|m| m.id = "../evil".into()), "lowercase letters"),
            (
                Box::new(|m| m.engine = "Whisper".into()),
                "lowercase letters",
            ),
            (Box::new(|m| m.id = String::new()), "lowercase letters"),
            (
                Box::new(|m| m.url = "http://example.invalid/a.bin".into()),
                "https",
            ),
            (
                Box::new(|m| m.url = "https://example.invalid/file".into()),
                "extension",
            ),
            (
                Box::new(|m| m.url = "https://example.invalid/file.".into()),
                "extension",
            ),
            (
                Box::new(|m| m.url = "https://example.invalid/f.b?x".into()),
                "extension",
            ),
            (Box::new(|m| m.sha256 = "abc".into()), "sha256"),
            (Box::new(|m| m.sha256 = m.sha256.to_uppercase()), "sha256"),
        ];
        for (mutate, expected) in cases {
            let mut m = model();
            mutate(&mut m);
            let err = Manifest::parse(&toml_entry(&m)).unwrap_err().to_string();
            assert!(err.contains(expected), "{err}");
        }
        let twice = toml_entry(&model()).repeat(2);
        assert!(
            Manifest::parse(&twice)
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
        let unknown = toml_entry(&model()) + "extra = 1\n";
        assert!(
            Manifest::parse(&unknown)
                .unwrap_err()
                .to_string()
                .starts_with("invalid model manifest")
        );
    }

    #[test]
    fn path_is_content_addressed() {
        let m = model();
        let p = m.path(Path::new("root"));
        assert_eq!(
            p,
            Path::new("root")
                .join("fake")
                .join(format!("test-model-{}.bin", &m.sha256[..12]))
        );
    }

    #[test]
    fn fetch_downloads_verifies_and_reuses() {
        let dir = Scratch::new("fetch");
        let calls = std::cell::Cell::new(0);
        let counting = |_: &str| -> Result<Box<dyn Read>, ModelError> {
            calls.set(calls.get() + 1);
            Ok(serve(DATA))
        };
        let path = fetch(&model(), &dir.0, &counting).unwrap();
        assert_eq!(fs::read(&path).unwrap(), DATA);
        // Present and intact: the opener is not called again.
        let again = fetch(&model(), &dir.0, &counting).unwrap();
        assert_eq!((again, calls.get()), (path, 1));
    }

    #[test]
    fn fetch_replaces_a_corrupt_file() {
        let dir = Scratch::new("corrupt");
        let path = model().path(&dir.0);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"garbage").unwrap();
        fetch(&model(), &dir.0, &serve_data).unwrap();
        assert_eq!(fs::read(&path).unwrap(), DATA);
    }

    #[test]
    fn fetch_rejects_wrong_size_hash_or_failure_and_leaves_nothing() {
        let dir = Scratch::new("reject");
        let short = fetch(&model(), &dir.0, &|_| Ok(serve(b"model"))).unwrap_err();
        assert_eq!(
            short.to_string(),
            format!("size is 5 bytes, expected {}", DATA.len())
        );
        let long = fetch(&model(), &dir.0, &|_| Ok(serve(b"model bytes and more"))).unwrap_err();
        assert!(matches!(long, ModelError::SizeMismatch { actual: 12, .. }));
        let wrong = fetch(&model(), &dir.0, &|_| Ok(serve(b"MODEL BYTES"))).unwrap_err();
        assert!(wrong.to_string().starts_with("sha256 is "));
        let failed = fetch(&model(), &dir.0, &|_| {
            Err(ModelError::Download("offline".into()))
        })
        .unwrap_err();
        assert_eq!(failed.to_string(), "download failed: offline");
        let engine_dir = model().path(&dir.0).parent().unwrap().to_owned();
        assert_eq!(
            fs::read_dir(engine_dir).unwrap().count(),
            0,
            "no .part left behind"
        );
    }

    #[test]
    fn fetch_reports_io_errors() {
        let dir = Scratch::new("io");
        // A file where the engine directory should be.
        fs::create_dir_all(&dir.0).unwrap();
        fs::write(dir.0.join("fake"), b"").unwrap();
        let err = fetch(&model(), &dir.0, &serve_data).unwrap_err();
        assert!(matches!(err, ModelError::Io(_)));
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn http_get_reads_a_response_and_reports_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for status in ["200 OK", "404 Not Found"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut req = [0; 1024];
                let _ = stream.read(&mut req).unwrap();
                let body: &[u8] = if status.starts_with("200") { DATA } else { b"" };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            }
        });
        let mut body = Vec::new();
        http_get(&format!("http://{addr}/m.bin"))
            .unwrap()
            .read_to_end(&mut body)
            .unwrap();
        assert_eq!(body, DATA);
        let err = http_get(&format!("http://{addr}/missing.bin"))
            .err()
            .unwrap();
        assert!(err.to_string().starts_with("download failed"), "{err}");
        server.join().unwrap();
    }
}
