use super::Fixture;
use std::collections::BTreeSet;
use tethera_common::protocol::error::WireError;
use tethera_common::structs::primitives::Cursor;
use tethera_common::structs::transcript::{Part, Role};
use tethera_server_lib::transcript::{PageBudget, TranscriptReader};

const TURNS: usize = 25;
const LIMIT: u16 = 10;

// The arithmetic the wire contract is stated in, and the reason the index is
// granular in turns rather than in records: this fixture is 49 records.
#[test]
fn three_pages_over_a_known_count_carry_every_turn_exactly_once() {
    let mut reader = Fixture::reader("paging.jsonl");

    let mut seen: Vec<String> = Vec::new();
    let mut earlier_was_false = 0;
    let mut pages = 0;
    let mut before: Option<Cursor> = None;

    loop {
        let page = reader.page(before.as_ref(), LIMIT).expect("a page");
        pages += 1;

        let mut ids: Vec<String> = page
            .items
            .iter()
            .map(|turn| turn.id.as_str().to_string())
            .collect();
        ids.extend(seen);
        seen = ids;

        if !page.has_earlier {
            earlier_was_false += 1;
        }

        match page.next_before {
            Some(cursor) => before = Some(cursor),
            None => break,
        }
    }

    assert_eq!(pages, 3);
    assert_eq!(seen.len(), TURNS);
    assert_eq!(
        seen.iter().collect::<BTreeSet<_>>().len(),
        TURNS,
        "a turn appeared on two pages"
    );
    assert_eq!(earlier_was_false, 1);
}

// A page is asked for as a count and delivered as one frame of bytes, and the
// two are independent. Before the byte bound, asking for a plausible number of
// real turns produced a frame the machine could not send — which reached the
// client as a stream that closed without answering, and left no line in any log
// at either end.
#[test]
fn a_page_of_turns_too_large_for_one_frame_is_cut_to_what_fits() {
    let mut reader = Fixture::reader("bulky.jsonl");
    let page = reader.page(None, LIMIT).expect("a page");

    assert!(
        !page.items.is_empty(),
        "a page that fits nothing at all would page forever without advancing"
    );
    assert!(
        page.items.len() < usize::from(LIMIT),
        "the count was the only bound; {} of {LIMIT} turns came back",
        page.items.len()
    );
    assert!(
        PageBudget::size_of(&page.items) <= PageBudget::MAX_PAGE_BYTES,
        "the page is {} bytes, over the {} it may occupy",
        PageBudget::size_of(&page.items),
        PageBudget::MAX_PAGE_BYTES
    );

    // What was cut has to be reachable, or the earlier turns are simply lost.
    assert!(page.has_earlier);
    assert!(page.next_before.is_some());
}

// The cut must not lose a turn or repeat one. `next_before` names the oldest
// turn that survived the budget, not the oldest the count asked for.
#[test]
fn paging_a_conversation_of_large_turns_still_carries_each_one_exactly_once() {
    let mut reader = Fixture::reader("bulky.jsonl");
    let total = reader.page(None, 1).map(|_| reader.turn_count()).expect("a count");

    let mut seen: Vec<String> = Vec::new();
    let mut before: Option<Cursor> = None;

    loop {
        let page = reader.page(before.as_ref(), LIMIT).expect("a page");

        let mut ids: Vec<String> = page
            .items
            .iter()
            .map(|turn| turn.id.as_str().to_string())
            .collect();
        ids.extend(seen);
        seen = ids;

        match page.next_before {
            Some(cursor) => before = Some(cursor),
            None => break,
        }
    }

    assert_eq!(seen.len(), total, "every turn in the fixture, once");
    assert_eq!(
        seen.iter().collect::<BTreeSet<_>>().len(),
        seen.len(),
        "a turn appeared on two pages"
    );
}

// Paging cannot help a single turn that alone exceeds the budget: the next page
// would carry the same turn and fail the same way. It is shrunk instead, so the
// conversation stays openable at that point in its history — and what was
// dropped is said out loud rather than silently missing.
#[test]
fn a_single_turn_too_large_for_any_frame_arrives_shortened_and_says_so() {
    let mut reader = Fixture::reader("colossal.jsonl");
    let page = reader.page(None, LIMIT).expect("a page");

    let overlarge = page
        .items
        .iter()
        .find(|turn| turn.role == Role::Agent)
        .expect("the agent's turn");

    assert!(
        PageBudget::size_of(overlarge) <= PageBudget::MAX_PAGE_BYTES,
        "the turn is still {} bytes",
        PageBudget::size_of(overlarge)
    );

    let notice = overlarge
        .parts
        .iter()
        .find_map(|part| match part {
            Part::Status { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .expect("a notice naming what could not be sent");

    assert!(
        notice.contains("too large"),
        "the notice has to say what happened: {notice}"
    );
}

// `next_before` is the first turn of the page just returned, and `before` is
// exclusive, so walking it never re-reads a turn and never skips one.
#[test]
fn the_next_cursor_is_the_first_turn_of_the_page_it_came_from() {
    let mut reader = Fixture::reader("paging.jsonl");

    let newest = reader.page(None, LIMIT).expect("a page");
    let boundary = newest.next_before.clone().expect("more to read");

    assert_eq!(boundary, newest.items[0].cursor);

    let older = reader.page(Some(&boundary), LIMIT).expect("a page");

    assert!(older
        .items
        .iter()
        .all(|turn| turn.cursor != newest.items[0].cursor));
}

// A group straddling a page boundary would arrive as two turns with different
// ids that no dedupe could rejoin. The index is granular in turns so the case
// cannot occur.
#[test]
fn a_page_boundary_never_splits_one_response() {
    let mut reader = Fixture::reader("paging.jsonl");
    let whole = reader.page(None, u16::MAX).expect("every turn");

    let mut paged = Vec::new();
    let mut before: Option<Cursor> = None;

    loop {
        let page = reader.page(before.as_ref(), 3).expect("a page");
        let mut items = page.items.clone();
        items.extend(paged);
        paged = items;

        match page.next_before {
            Some(cursor) => before = Some(cursor),
            None => break,
        }
    }

    assert_eq!(paged, whole.items);
}

#[test]
fn a_cursor_that_is_not_a_cursor_is_stale() {
    let mut reader = Fixture::reader("paging.jsonl");
    let refused = reader.page(Some(&Cursor("nonsense".into())), LIMIT);

    assert!(matches!(refused, Err(WireError::Stale)));
}

// Nothing is replayed: the client asked for the tail and already holds the page
// it just fetched.
#[test]
fn a_stream_with_no_cursor_opens_at_the_newest_turn() {
    let mut reader = Fixture::reader("paging.jsonl");
    let newest = reader.page(None, 1).expect("a page").items[0].cursor.clone();

    assert_eq!(reader.open_from(None).expect("a cursor"), newest);
}

#[test]
fn a_stream_resuming_at_a_known_turn_opens_exactly_there() {
    let mut reader = Fixture::reader("paging.jsonl");
    let page = reader.page(None, LIMIT).expect("a page");
    let held = page.items[0].cursor.clone();

    assert_eq!(reader.open_from(Some(&held)).expect("a cursor"), held);
}

// Later than what was asked for is the signal that tells a client to refetch the
// gap rather than render a history with a hole in it that it cannot see.
#[test]
fn a_cursor_that_resolves_to_nothing_opens_later_and_says_so() {
    let mut reader = Fixture::reader("paging.jsonl");
    let page = reader.page(None, LIMIT).expect("a page");
    let known = TranscriptReader::offset_of(&page.items[0].cursor).expect("an offset");

    // One byte inside the record, which is not a turn boundary. A rewritten file
    // produces exactly this.
    let stale = TranscriptReader::cursor_of(known - 1);
    let opened = reader.open_from(Some(&stale)).expect("a cursor");

    assert_eq!(opened, page.items[0].cursor);
    assert!(
        TranscriptReader::offset_of(&opened).expect("an offset") > known - 1,
        "the stream must not open earlier than it was asked to"
    );
}

// A cursor past the end belongs to a different file - a session resumed into a
// new one. Spec 19 defines client behaviour only for a `from` later than
// `after`, so an earlier answer would be a signal the client has no rule for.
#[test]
fn a_cursor_past_the_end_of_the_file_is_stale() {
    let mut reader = Fixture::reader("paging.jsonl");
    let beyond = TranscriptReader::cursor_of(u64::MAX / 2);

    assert!(matches!(
        reader.open_from(Some(&beyond)),
        Err(WireError::Stale)
    ));
}

#[test]
fn resuming_at_a_cursor_replays_only_what_came_after_it() {
    let mut reader = Fixture::reader("paging.jsonl");
    let page = reader.page(None, LIMIT).expect("a page");
    let fifth_back = page.items[LIMIT as usize - 5].cursor.clone();

    let replayed = reader.turns_after(&fifth_back).expect("the tail");

    assert_eq!(replayed.len(), 4);
    assert!(replayed.iter().all(|turn| turn.cursor != fifth_back));
    assert_eq!(replayed.last().expect("a turn").cursor, page.items[LIMIT as usize - 1].cursor);
}

// A tool result recorded long after the call it answers still reaches the turn
// that made it, because the index holds the whole file rather than a window.
#[test]
fn a_result_recorded_later_attaches_to_the_turn_that_called_it() {
    let mut reader = Fixture::reader("tools.jsonl");
    let page = reader.page(None, 100).expect("every turn");

    let completed: Vec<Option<String>> = page
        .items
        .iter()
        .flat_map(|turn| &turn.parts)
        .filter_map(|part| match part {
            tethera_common::structs::transcript::Part::ToolUse { result, .. } => {
                Some(result.clone())
            }
            _ => None,
        })
        .collect();

    assert_eq!(
        completed,
        vec![Some("hi".to_string()), Some("boom".to_string()), None]
    );
}
