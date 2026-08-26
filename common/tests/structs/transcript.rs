use tethera_common::structs::transcript::Part;

#[test]
fn an_unknown_part_falls_back_to_its_source_rows_verbatim() {
    let part = Part::Unknown {
        kind: "some_future_type".to_string(),
        fallback_text: "  raw   source rows  ".to_string(),
    };

    assert_eq!(part.fallback_text(), "  raw   source rows  ");
}

#[test]
fn a_text_part_falls_back_to_its_own_text() {
    let part = Part::Text {
        text: "hello".to_string(),
    };

    assert_eq!(part.fallback_text(), "hello");
}

#[test]
fn a_tool_use_part_falls_back_rather_than_exposing_its_input() {
    let part = Part::ToolUse {
        name: "Bash".to_string(),
        input: "rm -rf /".to_string(),
        fallback_text: "ran Bash".to_string(),
    };

    assert_eq!(part.fallback_text(), "ran Bash");
}
