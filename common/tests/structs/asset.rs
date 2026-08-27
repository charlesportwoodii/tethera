use tethera_common::structs::asset::{AssetCard, AssetScope};
use tethera_common::structs::ids::{AssetId, ConversationId, TabId};
use tethera_common::structs::primitives::Timestamp;

#[test]
fn an_asset_card_round_trips_through_postcard() {
    let card = AssetCard {
        asset: AssetId::parse("as_9f21ab").expect("valid"),
        name: "report.md".into(),
        mime: Some("text/markdown".into()),
        size: Some(4096),
        modified: Some(Timestamp(1_766_000_000_000)),
    };
    let bytes = postcard::to_stdvec(&card).expect("encode");

    assert_eq!(
        postcard::from_bytes::<AssetCard>(&bytes).expect("decode"),
        card
    );
}

// Absent is not zero. A size the scan did not report is drawn as missing, not as
// an empty file.
#[test]
fn an_unmeasured_asset_reports_no_size_rather_than_zero() {
    let card = AssetCard {
        asset: AssetId::parse("as_9f21ab").expect("valid"),
        name: "report.md".into(),
        mime: None,
        size: None,
        modified: None,
    };

    assert!(card.size.is_none());
    assert!(card.modified.is_none());
}

// Files are listed either for one conversation or for one tab's roots, and the
// two scopes must stay distinguishable on the wire.
#[test]
fn both_asset_scopes_round_trip() {
    for scope in [
        AssetScope::Conversation(ConversationId::parse("cv_9f21").expect("valid")),
        AssetScope::Tab(TabId::parse("tb_b2").expect("valid")),
    ] {
        let bytes = postcard::to_stdvec(&scope).expect("encode");

        assert_eq!(
            postcard::from_bytes::<AssetScope>(&bytes).expect("decode"),
            scope
        );
    }
}
