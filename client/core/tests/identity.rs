use tethera_client_core::identity::{Identity, MemoryStore, SecretStore};

// The identity is minted once and then reused. A second mint would change this
// device's endpoint id, which is the credential every paired machine holds.
#[test]
fn a_second_load_returns_the_key_the_first_one_minted() {
    let store = MemoryStore::new();

    let first = Identity::load_or_create(&store).expect("first load");
    let second = Identity::load_or_create(&store).expect("second load");

    assert_eq!(first.public(), second.public());
}

#[test]
fn the_minted_key_is_what_was_written_to_the_store() {
    let store = MemoryStore::new();

    let key = Identity::load_or_create(&store).expect("load");
    let stored = store
        .read(Identity::KEY_NAME)
        .expect("read")
        .expect("something was written");

    assert_eq!(stored, key.to_bytes().to_vec());
}

// The security invariant. Minting a fresh key over an unreadable one changes
// this device's identity, and every paired machine then answers NotEnrolled -
// which reads as "not paired" and sends a person to re-pair every machine they
// own, for a reason nothing ever showed them.
#[test]
fn a_stored_value_of_the_wrong_length_is_an_error_rather_than_a_new_identity() {
    let store = MemoryStore::new();
    store.seed(Identity::KEY_NAME, &[0u8; 16]);

    let result = Identity::load_or_create(&store);

    assert!(result.is_err(), "a short key must not be replaced silently");

    let after = store
        .read(Identity::KEY_NAME)
        .expect("read")
        .expect("still present");

    assert_eq!(
        after,
        vec![0u8; 16],
        "the stored value must not be overwritten"
    );
}
