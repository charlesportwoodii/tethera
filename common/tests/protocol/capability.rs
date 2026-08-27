use tethera_common::protocol::capability::{self, CapabilityId, CapabilitySet, HasCapability};

#[test]
fn a_capability_set_answers_for_a_name_it_holds() {
    let mut set = CapabilitySet::new();
    set.insert(CapabilityId::from(capability::TERMINAL_ATTACH));

    assert!(set.has(capability::TERMINAL_ATTACH));
    assert!(!set.has(capability::PUSH_FCM));
}

// A set of named ids rather than a struct of booleans, so adding a capability
// needs no wire change at any version. This test guards that property: the type
// of the field never changes.
#[test]
fn a_capability_from_a_newer_build_decodes_without_error() {
    let mut set = CapabilitySet::new();
    set.insert(CapabilityId::from("something_invented_later"));

    let bytes = postcard::to_stdvec(&set).expect("encode");
    let back: CapabilitySet = postcard::from_bytes(&bytes).expect("decode");

    assert!(back.has("something_invented_later"));
}

#[test]
fn every_capability_the_spec_names_is_declared() {
    let all = [
        capability::AGENT_CATALOG,
        capability::CONVERSATION_START,
        capability::CONVERSATION_RESUME,
        capability::CONVERSATION_STOP,
        capability::CONVERSATION_PREVIEW,
        capability::TRANSCRIPT_PAGING,
        capability::PROMPT_SEND,
        capability::INTERRUPT,
        capability::QUESTIONS,
        capability::QUESTIONS_PERMISSION,
        capability::RECENT_CWDS,
        capability::TERMINAL_ATTACH,
        capability::TERMINAL_INPUT,
        capability::TERMINAL_SCROLLBACK,
        capability::PANE_OPEN,
        capability::PANE_SPLIT,
        capability::PANE_CLOSE,
        capability::ASSETS_READ,
        capability::ASSETS_WRITE,
        capability::PUSH_FCM,
        capability::NOTIFY_POLICY,
        capability::DEVICE_SELF_REVOKE,
    ];

    assert_eq!(all.len(), 22);
    assert!(all.iter().all(|name| !name.is_empty()));
}

// Two capabilities sharing a name would make one of them unaskable: a set holds
// one entry and both features would report present together.
#[test]
fn no_two_capabilities_share_a_name() {
    let all = [
        capability::AGENT_CATALOG,
        capability::CONVERSATION_START,
        capability::CONVERSATION_RESUME,
        capability::CONVERSATION_STOP,
        capability::CONVERSATION_PREVIEW,
        capability::TRANSCRIPT_PAGING,
        capability::PROMPT_SEND,
        capability::INTERRUPT,
        capability::QUESTIONS,
        capability::QUESTIONS_PERMISSION,
        capability::RECENT_CWDS,
        capability::TERMINAL_ATTACH,
        capability::TERMINAL_INPUT,
        capability::TERMINAL_SCROLLBACK,
        capability::PANE_OPEN,
        capability::PANE_SPLIT,
        capability::PANE_CLOSE,
        capability::ASSETS_READ,
        capability::ASSETS_WRITE,
        capability::PUSH_FCM,
        capability::NOTIFY_POLICY,
        capability::DEVICE_SELF_REVOKE,
    ];

    let unique: CapabilitySet = all.iter().map(|name| CapabilityId::from(*name)).collect();

    assert_eq!(unique.len(), all.len());
}
