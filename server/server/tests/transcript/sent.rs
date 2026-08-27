use std::path::{Path, PathBuf};
use tethera_common::structs::transcript::Part;
use tethera_server_lib::transcript::SentFiles;

const UPLOADS: &str = r"C:\Users\charl\AppData\Local\alaydriem\tethera\data\uploads";

fn store() -> PathBuf {
    PathBuf::from(UPLOADS)
}

fn stored(name: &str) -> String {
    format!(r"{UPLOADS}\{name}")
}

// The shape `LiveConversations::naming` writes, and the whole point of this
// type: the person's words stay theirs, and the path becomes a card.
#[test]
fn a_prompt_with_an_attachment_splits_into_words_and_a_file() {
    let text = format!(
        "Retry after the index fix\n\nAttached: {}",
        stored("78d52cec5b5b-progress-probe.txt")
    );

    let (spoken, files) = SentFiles::split(&text, &store());

    assert_eq!(spoken, "Retry after the index fix");
    assert_eq!(files.len(), 1);
}

// A card and a raw path is the same file said twice, and the path is the half
// nobody can read on a phone.
#[test]
fn the_path_does_not_survive_in_the_words() {
    let text = format!("look at this\n\nAttached: {}", stored("aabbccddeeff-shot.png"));

    let (spoken, _) = SentFiles::split(&text, &store());

    assert!(!spoken.contains("Attached"), "{spoken:?}");
    assert!(!spoken.contains("uploads"), "{spoken:?}");
}

// Sending files with no message is ordinary. An empty bubble above the cards is
// not, so the words come back empty rather than as a blank line.
#[test]
fn a_prompt_that_was_only_files_leaves_no_words_behind() {
    let text = format!(
        "\nAttached: {}\nAttached: {}",
        stored("aabbccddeeff-one.png"),
        stored("112233445566-two.png")
    );

    let (spoken, files) = SentFiles::split(&text, &store());

    assert!(spoken.is_empty(), "{spoken:?}");
    assert_eq!(files.len(), 2);
}

// **The case that must not fire.** A person writing the word about a file of
// their own is prose, and eating it would delete something they typed — which,
// unlike a missing card, they cannot get back.
#[test]
fn a_person_writing_the_word_themselves_keeps_every_character() {
    for prose in [
        "Attached: see the notes I mailed you",
        r"Attached: C:\Users\charl\projects\tethera\README.md",
        "I have attached: nothing, actually",
        r"Attached: /home/dev/elsewhere/screenshot.png",
    ] {
        let (spoken, files) = SentFiles::split(prose, &store());

        assert_eq!(spoken, prose, "a person's own words were lifted out");
        assert!(files.is_empty());
    }
}

// A directory whose name merely starts with the store's is a different
// directory. Compared on a separator boundary rather than as a bare prefix,
// because `uploads-old` would otherwise read as inside `uploads`.
#[test]
fn a_sibling_directory_with_a_similar_name_is_not_the_store() {
    let text = format!("Attached: {UPLOADS}-old\\aabbccddeeff-shot.png");

    let (spoken, files) = SentFiles::split(&text, &store());

    assert_eq!(spoken, text);
    assert!(files.is_empty());
}

// The stored path has been through `canonicalize` and had Windows' extended
// prefix stripped; the directory comes from configuration. Separators and case
// genuinely differ between them, and a miss here loses the card silently.
#[test]
fn the_store_is_recognised_however_the_path_is_spelled() {
    let store = store();

    for spelling in [
        format!(r"{UPLOADS}\aabbccddeeff-shot.png"),
        format!("{}/aabbccddeeff-shot.png", UPLOADS.replace('\\', "/")),
        format!(r"{}\aabbccddeeff-shot.png", UPLOADS.to_lowercase()),
    ] {
        let text = format!("here\n\nAttached: {spelling}");
        let (_, files) = SentFiles::split(&text, &store);

        assert_eq!(files.len(), 1, "{spelling} was not read as the store");
    }
}

// The digest in front of a stored name exists so two people sending
// `screenshot.png` do not overwrite each other. It is storage, not identity, and
// a card showing it names the file something nobody chose.
#[test]
fn a_stored_file_is_shown_under_the_name_the_person_gave_it() {
    let path = PathBuf::from(stored("78d52cec5b5b-progress-probe.txt"));

    assert_eq!(SentFiles::readable_name(&path), "progress-probe.txt");
}

// And only the exact shape the store writes. A file genuinely named with a
// hyphenated first word keeps it, or a person loses a word from their filename.
#[test]
fn a_hyphenated_name_that_is_not_a_digest_keeps_its_first_word() {
    for name in [
        "release-notes.md",
        "2026-08-26-report.pdf",
        "zzzzzzzzzzzz-notdigest.txt",
        "78d52cec5b5-tooshort.txt",
    ] {
        let path = PathBuf::from(stored(name));

        assert_eq!(SentFiles::readable_name(&path), name, "{name} lost a word");
    }
}

// A prompt with nothing to lift must come back byte for byte. This runs on every
// operator turn ever read, so the untouched path is the common one.
#[test]
fn a_prompt_with_no_attachment_is_returned_unchanged() {
    let text = "just a message\n\nwith a blank line in it\n";

    let (spoken, files) = SentFiles::split(text, &store());

    assert_eq!(spoken, text);
    assert!(files.is_empty());
}

// A store that is nowhere admits nothing. Otherwise an empty directory would
// prefix-match every path on the machine and lift the lot.
#[test]
fn an_empty_store_recognises_nothing() {
    let text = format!("here\n\nAttached: {}", stored("aabbccddeeff-shot.png"));

    let (spoken, files) = SentFiles::split(&text, Path::new(""));

    assert_eq!(spoken, text);
    assert!(files.is_empty());
}

// The contract a client draws against, end to end through the real reader: the
// person's words are the bubble and the cards sit under them, in that order.
#[test]
fn a_persons_turn_carries_their_words_then_their_files() {
    let turns = read_attached();
    let parts = &turns[0].parts;

    assert_eq!(parts.len(), 2, "{parts:?}");
    assert!(matches!(&parts[0], Part::Text { text } if text == "Retry after the index fix"));

    let Part::File { name, mime, .. } = &parts[1] else {
        panic!("the attachment did not become a card: {parts:?}");
    };

    assert_eq!(name, "progress-probe.txt");
    assert_eq!(mime.as_deref(), Some("text/plain"));
}

// Files with no message: cards and no empty bubble above them.
#[test]
fn a_turn_that_was_only_a_file_is_only_a_card() {
    let turns = read_attached();
    let parts = &turns[1].parts;

    assert_eq!(parts.len(), 1, "{parts:?}");

    let Part::File { name, mime, .. } = &parts[0] else {
        panic!("expected a card: {parts:?}");
    };

    assert_eq!(name, "screenshot.png");
    assert_eq!(mime.as_deref(), Some("image/png"));
}

// And the words a person typed themselves, reaching the transcript untouched.
#[test]
fn a_person_who_typed_the_word_gets_their_sentence_back() {
    let turns = read_attached();
    let parts = &turns[2].parts;

    assert_eq!(parts.len(), 1, "{parts:?}");
    assert!(
        matches!(&parts[0], Part::Text { text } if text == "Attached: the notes I mailed you, not a file"),
        "{parts:?}"
    );
}

/// The fixture read through the real reader, with the store the fixture names.
fn read_attached() -> Vec<tethera_common::structs::transcript::Turn> {
    use tethera_common::structs::transcript::Role;

    read_all()
        .into_iter()
        .filter(|turn| turn.role == Role::Operator)
        .collect()
}

/// Every turn in the fixture, read through the real reader with the store it
/// names, so the mapper wiring is exercised rather than the split alone.
fn read_all() -> Vec<tethera_common::structs::transcript::Turn> {
    use tethera_common::structs::agent::Agent;
    use tethera_server_lib::transcript::{AssetIndex, TranscriptReader};

    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("transcripts")
        .join("attached.jsonl");

    let mut reader = TranscriptReader::indexing(
        path,
        Agent::Claude,
        AssetIndex::new_shared(),
        PathBuf::from("/var/store/uploads"),
    );

    reader.page(None, u16::MAX).expect("page").items
}
