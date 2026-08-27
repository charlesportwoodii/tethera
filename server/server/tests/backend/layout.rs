use tethera_common::structs::ids::TabId;
use tethera_server_lib::backend::herdr::{wire, HerdrIds, Mapping};

fn a_herdr_layout() -> wire::PaneLayout {
    wire::PaneLayout {
        workspace_id: "w6H".into(),
        tab_id: "w6H:t1".into(),
        zoomed: false,
        area: wire::Rect {
            x: 29,
            y: 1,
            width: 120,
            height: 40,
        },
        focused_pane_id: "w6H:p1".into(),
        panes: vec![
            wire::LayoutPane {
                pane_id: "w6H:p1".into(),
                focused: true,
                rect: wire::Rect {
                    x: 29,
                    y: 1,
                    width: 60,
                    height: 40,
                },
            },
            wire::LayoutPane {
                pane_id: "w6H:p2".into(),
                focused: false,
                rect: wire::Rect {
                    x: 89,
                    y: 1,
                    width: 60,
                    height: 40,
                },
            },
        ],
        splits: Vec::new(),
    }
}

// herdr reports a tab that starts at column 29, because the desk has a sidebar.
// Carried through as herdr states it: the client normalises against the union
// of the slots, so shifting the origin here would make a one-pane tab and a
// two-pane tab disagree about where the origin is.
#[test]
fn a_layout_keeps_the_coordinates_herdr_reported() {
    let tab = TabId::parse("tb_one").expect("valid");

    let mapped = Mapping::layout(&a_herdr_layout(), &tab);

    assert_eq!(mapped.slots.len(), 2);
    assert_eq!(mapped.slots[0].rect.x, 29);
    assert_eq!(mapped.slots[1].rect.x, 89);
}

// herdr reports the zoom as a flag and the pane as a separate always-present
// string, so `focused_pane_id` names a pane whether or not anything is zoomed.
// Ours carries the zoomed pane instead, so a false flag has to become `None`
// and never `Some(focused)`.
#[test]
fn an_unzoomed_tab_names_no_zoomed_pane() {
    let tab = TabId::parse("tb_one").expect("valid");

    let mapped = Mapping::layout(&a_herdr_layout(), &tab);

    assert_eq!(mapped.zoomed, None);
}

#[test]
fn a_zoomed_tab_names_the_pane_that_is_zoomed() {
    let mut herdr = a_herdr_layout();
    herdr.zoomed = true;
    let tab = TabId::parse("tb_one").expect("valid");

    let mapped = Mapping::layout(&herdr, &tab);

    assert_eq!(mapped.zoomed, Some(HerdrIds::pane("w6H:p1")));
}

// `focused_pane_id` is a plain string with no empty case in the schema, so an
// empty or unplaced one is herdr disagreeing with itself. Naming a pane the
// layout does not place would zoom the map onto a rectangle that is not on it.
#[test]
fn a_zoom_naming_a_pane_the_layout_does_not_place_is_no_zoom() {
    let mut herdr = a_herdr_layout();
    herdr.zoomed = true;
    herdr.focused_pane_id = String::new();
    let tab = TabId::parse("tb_one").expect("valid");

    let mapped = Mapping::layout(&herdr, &tab);

    assert_eq!(mapped.zoomed, None);
}

// A tab herdr placed nothing in draws nothing rather than an empty box, so the
// mapping has to survive an empty pane list rather than assume a first pane.
#[test]
fn a_tab_with_no_placed_panes_maps_to_no_slots() {
    let mut herdr = a_herdr_layout();
    herdr.panes.clear();
    let tab = TabId::parse("tb_one").expect("valid");

    let mapped = Mapping::layout(&herdr, &tab);

    assert!(mapped.slots.is_empty());
    assert_eq!(mapped.zoomed, None);
}
