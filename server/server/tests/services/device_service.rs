//! Resolving the device an operator meant.
//!
//! `revoke`, `ban` and `unban` are all built on this, so resolving the wrong
//! device is the difference between locking out a lost phone and locking out
//! the operator's own.

use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tethera_server_lib::config::ApplicationConfig;
use tethera_server_lib::services::DeviceService;
use tethera_server_lib::storage::Storage;

const ONE: &str = "aa11bb22cc33dd44ee55ff6607788990aa11bb22cc33dd44ee55ff6607788990";
const TWO: &str = "aa11bb22cc33dd44ee55ff660778899000000000000000000000000000000000";
const OTHER: &str = "ffffffff0000111122223333444455556666777788889999aaaabbbbccccdddd";

struct Fixture {
    _dir: tempfile::TempDir,
    db: Arc<DatabaseConnection>,
}

impl Fixture {
    async fn start() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = ApplicationConfig::with_data_dir(dir.path().to_path_buf());
        let db = Arc::new(Storage::connect(&config).await.expect("storage"));

        Self { _dir: dir, db }
    }

    fn devices(&self) -> DeviceService {
        DeviceService::new(self.db.clone())
    }

    async fn enrol(&self, endpoint_id: &str, name: &str) {
        self.devices()
            .activate(self.db.as_ref(), endpoint_id, name, 1_766_000_000)
            .await
            .expect("enrolled");
    }
}

#[tokio::test]
async fn a_whole_endpoint_id_resolves_to_its_device() {
    let fixture = Fixture::start().await;
    fixture.enrol(ONE, "phone").await;

    let found = fixture
        .devices()
        .resolve(fixture.db.as_ref(), ONE)
        .await
        .expect("resolved");

    assert_eq!(found.endpoint_id, ONE);
}

// A 64-character endpoint id is not something anybody types, so a unique prefix
// resolves the way a short commit hash does.
#[tokio::test]
async fn a_unique_prefix_resolves_to_its_device() {
    let fixture = Fixture::start().await;
    fixture.enrol(ONE, "phone").await;
    fixture.enrol(OTHER, "tablet").await;

    let found = fixture
        .devices()
        .resolve(fixture.db.as_ref(), "ffff")
        .await
        .expect("resolved");

    assert_eq!(found.name, "tablet");
}

// The commands built on this revoke and ban. Picking the first match would
// eventually lock somebody out of their own machine, so an ambiguous prefix is
// refused and the candidates are named.
#[tokio::test]
async fn an_ambiguous_prefix_is_refused_and_names_what_it_matched() {
    let fixture = Fixture::start().await;
    fixture.enrol(ONE, "phone").await;
    fixture.enrol(TWO, "spare").await;

    let error = fixture
        .devices()
        .resolve(fixture.db.as_ref(), "aa11bb22")
        .await
        .expect_err("an ambiguous prefix resolved to one device");

    let message = format!("{error}");

    assert!(message.contains(ONE), "{message}");
    assert!(message.contains(TWO), "{message}");
}

#[tokio::test]
async fn a_prefix_matching_nothing_is_refused() {
    let fixture = Fixture::start().await;
    fixture.enrol(ONE, "phone").await;

    assert!(fixture
        .devices()
        .resolve(fixture.db.as_ref(), "0123")
        .await
        .is_err());
}

// An empty needle is a prefix of every endpoint id, so without its own refusal
// it would resolve to whichever device happened to be first.
#[tokio::test]
async fn an_empty_identifier_is_refused_rather_than_matching_everything() {
    let fixture = Fixture::start().await;
    fixture.enrol(ONE, "phone").await;

    assert!(fixture
        .devices()
        .resolve(fixture.db.as_ref(), "")
        .await
        .is_err());
}
