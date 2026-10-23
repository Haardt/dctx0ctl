//! Storage encodings never change public JSON or content identities.
use super::*;
use flate2::{Compression, GzBuilder, read::GzDecoder};
use serde::ser::SerializeStruct;
use std::io::{BufReader, Read, Write};

pub(crate) struct HashWriter<W> {
    pub output: W,
    pub hash: Sha256,
    pub length: u64,
}
impl<W> HashWriter<W> {
    pub fn new(output: W) -> Self {
        Self {
            output,
            hash: Sha256::new(),
            length: 0,
        }
    }
}
impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self.output.write(bytes)?;
        self.hash.update(&bytes[..count]);
        self.length += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.output.flush()
    }
}
pub(crate) fn json_hash(value: &impl Serialize) -> Result<String> {
    let mut writer = HashWriter::new(std::io::sink());
    serde_json::to_writer(&mut writer, value)?;
    Ok(format!("{:x}", writer.hash.finalize()))
}
pub(crate) fn compress_json(value: &impl Serialize) -> Result<(String, u64, Vec<u8>)> {
    let encoder = GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), Compression::default());
    let mut writer = HashWriter::new(encoder);
    serde_json::to_writer(&mut writer, value)?;
    Ok((
        format!("{:x}", writer.hash.finalize()),
        writer.length,
        writer.output.finish()?,
    ))
}
pub(crate) fn encode(bytes: &[u8]) -> Result<(&'static str, Vec<u8>)> {
    if bytes.len() >= 4096 {
        let mut encoder = GzBuilder::new()
            .mtime(0)
            .operating_system(255)
            .write(Vec::new(), Compression::default());
        encoder.write_all(bytes)?;
        let compressed = encoder.finish()?;
        if compressed.len() < bytes.len() {
            return Ok(("gzip", compressed));
        }
    }
    Ok(("identity", bytes.to_vec()))
}
pub(crate) fn decode_bytes(encoding: &str, bytes: Vec<u8>, length: Option<i64>) -> Result<Vec<u8>> {
    let decoded = match encoding {
        "identity" => bytes,
        "gzip" => {
            let length = length
                .filter(|n| *n >= 0)
                .ok_or_else(|| invalid("Compressed content needs its original length"))?;
            let mut decoded = Vec::new();
            // Bound expansion by the declared size, including corrupt gzip input.
            GzDecoder::new(bytes.as_slice())
                .take((length as u64).saturating_add(1))
                .read_to_end(&mut decoded)?;
            decoded
        }
        _ => return Err(invalid("Unsupported verification storage encoding")),
    };
    if length.is_some_and(|length| length != decoded.len() as i64) {
        return Err(invalid("Stored verification content length is corrupt"));
    }
    Ok(decoded)
}
pub(crate) struct Header<'a>(pub &'a VerificationRun);
impl Serialize for Header<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let run = self.0;
        let mut s = serializer.serialize_struct("VerificationRun", 15)?;
        macro_rules! field {
            ($name:ident) => {
                s.serialize_field(stringify!($name), &run.$name)?
            };
        }
        field!(schema_version);
        field!(origin);
        field!(started_at_unix_ns);
        field!(elapsed_us);
        field!(provenance);
        field!(budgets);
        field!(selected_subjects);
        field!(required_subjects);
        field!(not_selected);
        s.serialize_field("required_cases", &[] as &[String])?;
        s.serialize_field("cases", &[] as &[dctx0_core::VerificationCaseRecord])?;
        field!(open_obligations);
        field!(bounded_obligations);
        field!(artifacts);
        field!(result);
        s.end()
    }
}
#[derive(Serialize)]
pub(crate) struct Corpus<'a> {
    pub cases: &'a [dctx0_core::VerificationCaseRecord],
    pub required_cases: &'a BTreeSet<String>,
}
#[derive(Deserialize)]
pub(crate) struct OwnedCorpus {
    pub cases: Vec<dctx0_core::VerificationCaseRecord>,
    pub required_cases: BTreeSet<String>,
}
struct HashReader<R> {
    input: R,
    hash: Sha256,
    length: u64,
}
impl<R: Read> Read for HashReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.input.read(bytes)?;
        self.hash.update(&bytes[..count]);
        self.length += count as u64;
        Ok(count)
    }
}
impl<R> HashReader<R> {
    fn verify(self, hash: &str, length: i64) -> Result<()> {
        if self.length != length as u64 || format!("{:x}", self.hash.finalize()) != hash {
            return Err(invalid("Stored case corpus digest or length is corrupt"));
        }
        Ok(())
    }
}
fn corpus_reader(bytes: &[u8], length: i64) -> Result<HashReader<impl Read + '_>> {
    if length < 0 {
        return Err(invalid("Stored case corpus length is corrupt"));
    }
    Ok(HashReader {
        input: GzDecoder::new(bytes).take(length as u64 + 1),
        hash: Sha256::new(),
        length: 0,
    })
}
pub(crate) fn validate_corpus(bytes: &[u8], length: i64, hash: &str) -> Result<()> {
    let mut reader = corpus_reader(bytes, length)?;
    std::io::copy(&mut reader, &mut std::io::sink())?;
    reader.verify(hash, length)
}
fn read_corpus(bytes: &[u8], length: i64, hash: &str) -> Result<OwnedCorpus> {
    let mut reader = corpus_reader(bytes, length)?;
    let mut de = serde_json::Deserializer::from_reader(BufReader::new(&mut reader));
    de.disable_recursion_limit();
    let corpus = OwnedCorpus::deserialize(serde_stacker::Deserializer::new(&mut de))?;
    de.end()?;
    drop(de);
    reader.verify(hash, length)?;
    Ok(corpus)
}

/// Shared by publication and task-proof readers, including legacy schema 4.
pub(crate) fn read_run(
    connection: &Connection,
    project: &str,
    id: &str,
) -> Result<StoredVerificationRun> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let sql = if version >= 5 {
        "SELECT created_at,data,corpus_hash,pruned_at FROM verification_runs WHERE project=?1 AND id=?2"
    } else {
        "SELECT created_at,data,NULL,NULL FROM verification_runs WHERE project=?1 AND id=?2"
    };
    let row = connection
        .query_row(sql, params![project, id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .optional()?
        .ok_or_else(|| Error::Missing(format!("Verification run {id}")))?;
    if row.3.is_some() {
        return Err(Error::DetailsPruned(id.into()));
    }
    let mut run: VerificationRun = decode(&row.1)?;
    if let Some(hash) = row.2 {
        if !run.cases.is_empty() || !run.required_cases.is_empty() {
            return Err(invalid("Stored run header contains a second case corpus"));
        }
        let (bytes, length): (Vec<u8>, i64) = connection.query_row(
            "SELECT bytes,raw_length FROM verification_corpora WHERE hash=?1",
            [&hash],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let corpus = read_corpus(&bytes, length, &hash)?;
        run.cases = corpus.cases;
        run.required_cases = corpus.required_cases;
    }
    if version >= 6 && cases::prepared(connection, id)? {
        cases::validate_header(&run)?;
        run.required_cases = cases::requirements(connection, id)?;
        run.cases = cases::read_all(connection, id)?;
        if json_hash(&run)? != id {
            return Err(invalid("Reconstructed run digest is corrupt"));
        }
    }
    Ok(StoredVerificationRun {
        id: id.into(),
        created_at: row.0,
        run,
    })
}
pub(crate) fn read_artifact(
    connection: &Connection,
    run_id: &str,
    hash: &str,
) -> Result<VerificationArtifact> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let sql = if version >= 5 {
        "SELECT a.media_type,a.bytes,a.encoding,a.raw_length FROM verification_artifacts a JOIN verification_run_artifacts r ON r.artifact_hash=a.hash WHERE r.run_id=?1 AND a.hash=?2"
    } else {
        "SELECT a.media_type,a.bytes,'identity',NULL FROM verification_artifacts a JOIN verification_run_artifacts r ON r.artifact_hash=a.hash WHERE r.run_id=?1 AND a.hash=?2"
    };
    let (media, bytes, encoding, length): (String, Vec<u8>, String, Option<i64>) = connection
        .query_row(sql, params![run_id, hash], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()?
        .ok_or_else(|| Error::Missing(format!("Verification artifact {hash}")))?;
    let artifact = VerificationArtifact::new(&media, decode_bytes(&encoding, bytes, length)?);
    if artifact.reference.hash != hash {
        return Err(invalid("Stored artifact digest is corrupt"));
    }
    Ok(artifact)
}
