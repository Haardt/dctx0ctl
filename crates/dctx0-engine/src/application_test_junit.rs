//! Closed Bun parent-result validation. Native exit status and the central case
//! journal are independent gates; a JUnit success alone grants no authority.
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use std::collections::BTreeMap;

const LIMIT: usize = 1_048_576;

fn attributes(
    start: &BytesStart<'_>,
    reader: &Reader<&[u8]>,
    allowed: &[&str],
) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute.map_err(|error| format!("Invalid JUnit attribute: {error}"))?;
        let name = std::str::from_utf8(attribute.key.as_ref())
            .map_err(|_| "JUnit attribute is not UTF-8")?;
        if !allowed.contains(&name) {
            return Err(format!("Unsupported JUnit attribute {name}"));
        }
        let value = attribute
            .decode_and_unescape_value(reader.decoder())
            .map_err(|error| format!("Invalid JUnit attribute value: {error}"))?
            .into_owned();
        if value
            .chars()
            .any(|value| value.is_control() && !matches!(value, '\t' | '\n' | '\r'))
        {
            return Err("Forbidden XML control character".into());
        }
        result.insert(name.to_owned(), value);
    }
    Ok(result)
}

fn counts(values: &BTreeMap<String, String>) -> Result<(), String> {
    for (name, expected) in [("tests", "1"), ("failures", "0"), ("skipped", "0")] {
        if values.get(name).map(String::as_str) != Some(expected) {
            return Err(format!("JUnit requires {name}={expected}"));
        }
    }
    if values.get("errors").is_some_and(|value| value != "0") {
        return Err("JUnit reports errors".into());
    }
    Ok(())
}

/// Accept exactly one successful, non-skipped native parent from Bun's JUnit
/// shape. XML is parsed by quick-xml; DTDs, entity declarations, instructions,
/// unknown shapes and duplicate attributes are rejected before acceptance.
pub(crate) fn assess(bytes: &[u8], root_test: &str) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > LIMIT || root_test.is_empty() || root_test.len() > 4096 {
        return Err("Invalid JUnit input size or native parent identity".into());
    }
    std::str::from_utf8(bytes).map_err(|_| "JUnit is not UTF-8")?;
    if bytes
        .iter()
        .any(|value| *value < 32 && !matches!(*value, 9 | 10 | 13))
    {
        return Err("Forbidden XML control character".into());
    }
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().check_comments = true;
    let mut stack: Vec<&str> = vec![];
    let (mut roots, mut suites, mut cases) = (0, 0, 0);
    let mut first = true;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| format!("Malformed JUnit XML: {error}"))?;
        match &event {
            Event::Start(start) | Event::Empty(start) => {
                let name = match (stack.as_slice(), start.name().as_ref()) {
                    ([], b"testsuites") if roots == 0 => {
                        roots += 1;
                        "testsuites"
                    }
                    (["testsuites"], b"testsuite") if suites == 0 => {
                        suites += 1;
                        "testsuite"
                    }
                    (["testsuites", "testsuite"], b"testcase") if cases == 0 => {
                        cases += 1;
                        "testcase"
                    }
                    _ => return Err("Unexpected, duplicate or failed JUnit element".into()),
                };
                let allowed = if name == "testcase" {
                    &["name", "classname", "time", "file", "line", "assertions"][..]
                } else {
                    &[
                        "name",
                        "tests",
                        "failures",
                        "errors",
                        "skipped",
                        "time",
                        "assertions",
                        "file",
                        "hostname",
                    ][..]
                };
                let values = attributes(start, &reader, allowed)?;
                if name == "testcase" {
                    if values.get("name").map(String::as_str) != Some(root_test) {
                        return Err(
                            "JUnit parent identity differs from the required native test".into(),
                        );
                    }
                } else {
                    counts(&values)?;
                }
                if matches!(event, Event::Start(_)) {
                    stack.push(name);
                }
            }
            Event::End(end) => {
                if stack.pop().map(str::as_bytes) != Some(end.name().as_ref()) {
                    return Err("Mismatched JUnit element".into());
                }
            }
            Event::Decl(declaration) if first => {
                if declaration
                    .version()
                    .map_err(|error| error.to_string())?
                    .as_ref()
                    != b"1.0"
                {
                    return Err("Unsupported JUnit XML version".into());
                }
                let content =
                    std::str::from_utf8(declaration).map_err(|_| "Invalid XML declaration")?;
                let start = BytesStart::from_content(content, 3);
                let values = attributes(&start, &reader, &["version", "encoding", "standalone"])?;
                if values
                    .get("encoding")
                    .is_some_and(|value| !value.eq_ignore_ascii_case("utf-8"))
                    || values
                        .get("standalone")
                        .is_some_and(|value| value != "yes" && value != "no")
                {
                    return Err("Unsupported JUnit encoding or standalone declaration".into());
                }
            }
            Event::Text(text) if text.as_ref().iter().all(u8::is_ascii_whitespace) => {}
            Event::Comment(_) => {}
            Event::Eof => break,
            _ => return Err("Unsupported XML content in JUnit".into()),
        }
        first = false;
    }
    if !stack.is_empty() || (roots, suites, cases) != (1, 1, 1) {
        return Err("JUnit parent execution is incomplete".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuites tests=\"1\" failures=\"0\" skipped=\"0\"><testsuite tests=\"1\" failures=\"0\" skipped=\"0\"><testcase name=\"CinemaSuite\"/></testsuite></testsuites>";
    #[test]
    fn accepts_bounded_bun_parent_and_decodes_standard_attribute_entities() {
        assert!(assess(VALID.as_bytes(), "CinemaSuite").is_ok());
        assert!(
            assess(
                VALID.replace("CinemaSuite", "Cinema&amp;Suite").as_bytes(),
                "Cinema&Suite"
            )
            .is_ok()
        );
        assert!(
            assess(
                VALID
                    .replace(
                        "<testcase name=\"CinemaSuite\"/>",
                        "<testcase name=\"CinemaSuite\"> <!-- native parent --> </testcase>"
                    )
                    .as_bytes(),
                "CinemaSuite"
            )
            .is_ok()
        );
    }
    #[test]
    fn rejects_missing_duplicate_skipped_failed_or_wrong_parent() {
        for changed in [
            VALID.replace("name=\"CinemaSuite\"", "name=\"Other\""),
            VALID.replace("<testcase name=\"CinemaSuite\"/>", ""),
            VALID.replace(
                "<testcase name=\"CinemaSuite\"/>",
                "<testcase name=\"CinemaSuite\"/><testcase name=\"CinemaSuite\"/>",
            ),
            VALID.replace(
                "<testcase name=\"CinemaSuite\"/>",
                "<testcase name=\"CinemaSuite\"><failure/></testcase>",
            ),
            VALID.replace(
                "<testcase name=\"CinemaSuite\"/>",
                "<testcase name=\"CinemaSuite\"><skipped/></testcase>",
            ),
            VALID.replace("tests=\"1\"", "tests=\"2\""),
            VALID.replace("failures=\"0\"", "failures=\"1\""),
            VALID.replace("skipped=\"0\"", "skipped=\"1\""),
            VALID.replace("failures=\"0\"", "failures=\"0\" errors=\"1\""),
        ] {
            assert!(
                assess(changed.as_bytes(), "CinemaSuite").is_err(),
                "{changed}"
            );
        }
    }
    #[test]
    fn rejects_malformed_unsafe_xml_unknown_forms_and_encoding() {
        for changed in [
            VALID.replace("tests=\"1\"", "tests=\"1\" tests=\"1\""),
            VALID.replace("name=\"CinemaSuite\"", "name=\"CinemaSuite\" unknown=\"x\""),
            VALID.replace("</testsuite>", "</wrong>"),
            VALID.replace("UTF-8", "UTF-16"),
            VALID.replace("version=\"1.0\"", "version=\"1.1\""),
            VALID.replace(
                "<testsuites",
                "<!DOCTYPE x SYSTEM 'file:///tmp/private'><testsuites",
            ),
            VALID.replace("<testcase", "<?command unsafe?><testcase"),
            VALID.replace("<testcase name=\"CinemaSuite\"/>", "<![CDATA[pass]]>"),
            VALID.replace("CinemaSuite", "&external;"),
            VALID.replace("</testsuite>", "&external;</testsuite>"),
            VALID.replace("</testsuite>", "non-whitespace</testsuite>"),
            VALID.replace("</testsuite>", "<!-- invalid -- comment --></testsuite>"),
            VALID.replace("1.0\" encoding", "1.0\" version=\"1.0\" encoding"),
            format!("{VALID}{VALID}"),
        ] {
            assert!(
                assess(changed.as_bytes(), "CinemaSuite").is_err(),
                "{changed}"
            );
        }
        assert!(assess(&[255], "CinemaSuite").is_err());
        assert!(assess(VALID.as_bytes(), "").is_err());
        assert!(assess(&vec![b' '; LIMIT + 1], "CinemaSuite").is_err());
        assert!(
            assess(
                VALID.replace("CinemaSuite", "Cinema\0Suite").as_bytes(),
                "CinemaSuite"
            )
            .is_err()
        );
    }
    #[test]
    fn xml_declaration_requires_version_and_valid_standalone_flag() {
        for declaration in [
            "<?xml encoding=\"UTF-8\"?>",
            "<?xml version=\"1.0\" standalone=\"maybe\"?>",
            "<?xml version=\"1.0\" standalone=\"yes\" standalone=\"no\"?>",
        ] {
            let body = VALID.split_once("?>").unwrap().1;
            assert!(assess(format!("{declaration}{body}").as_bytes(), "CinemaSuite").is_err());
        }
        let standalone = VALID.replace(" encoding=\"UTF-8\"", " standalone=\"yes\"");
        assert!(assess(standalone.as_bytes(), "CinemaSuite").is_ok());
    }
}
