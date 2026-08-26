use tethera_relay::access::SharedSecretAccess;

#[test]
fn the_configured_token_is_admitted() {
    let access = SharedSecretAccess::new("s3cr3t-token".to_string());

    assert!(access.admits(Some("s3cr3t-token")));
}

#[test]
fn a_wrong_token_is_refused() {
    let access = SharedSecretAccess::new("s3cr3t-token".to_string());

    assert!(!access.admits(Some("wrong-token")));
}

#[test]
fn an_absent_token_is_refused() {
    let access = SharedSecretAccess::new("s3cr3t-token".to_string());

    assert!(!access.admits(None));
}

#[test]
fn a_prefix_of_the_right_token_is_refused() {
    let access = SharedSecretAccess::new("s3cr3t-token".to_string());

    assert!(!access.admits(Some("s3cr3t")));
    assert!(!access.admits(Some("")));
}

#[test]
fn a_longer_string_beginning_with_the_right_token_is_refused() {
    let access = SharedSecretAccess::new("s3cr3t-token".to_string());

    assert!(!access.admits(Some("s3cr3t-token-and-more")));
}
