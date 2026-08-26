use std::io::Write;
use tethera_relay::access::SharedSecretAccess;
use tethera_relay::config::RelayConfig;

struct Fixture;

impl Fixture {
    fn write(body: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().expect("tempfile");
        write!(file, "{body}").expect("write");
        file
    }
}

#[test]
fn the_secret_that_validates_is_the_secret_that_is_enforced() {
    let file = Fixture::write("secret = \"  s3cr3t-token  \"\n");

    let config = RelayConfig::from_file(file.path()).expect("a valid config must load");
    config.validate().expect("a non-empty secret must validate");

    let access = SharedSecretAccess::new(config.secret.clone());

    assert!(
        access.admits(Some("s3cr3t-token")),
        "the trimmed value validated, so the trimmed value must be admitted"
    );
    assert!(
        !access.admits(Some("  s3cr3t-token  ")),
        "the untrimmed value must not be what the relay enforces"
    );
}

#[test]
fn a_whitespace_only_secret_is_refused_at_startup() {
    let file = Fixture::write("secret = \"   \"\n");

    let config = RelayConfig::from_file(file.path()).expect("a parseable config must load");

    assert!(config.validate().is_err());
}

#[test]
fn a_malformed_config_reports_its_line_without_quoting_the_source() {
    let file = Fixture::write("http_bind = \"0.0.0.0:8080\"\nsecret = super-secret-value\n");

    let error = RelayConfig::from_file(file.path())
        .expect_err("a malformed config must not load")
        .to_string();

    assert!(error.contains("line 2"), "unexpected message: {error}");
    assert!(
        !error.contains("super-secret-value"),
        "the message must not carry the offending source line: {error}"
    );
}
