use tethera_server_lib::transcript::AssetNaming;

// The id is minted from the canonical spelling and must keep it. What must not
// keep it is the path handed to an agent inside a prompt: `\\?\` reaches some
// tools that refuse it, and it is noise to anybody reading their own transcript.
#[test]
fn a_path_for_a_person_loses_the_extended_length_prefix() {
    assert_eq!(
        AssetNaming::plain(r"\\?\C:\Users\charl\uploads\notes.txt"),
        r"C:\Users\charl\uploads\notes.txt"
    );
}

// A share reached through the prefix has a plain spelling of its own, and it is
// not the one that falls out of dropping four characters.
#[test]
fn a_share_becomes_the_share_rather_than_a_broken_path() {
    assert_eq!(
        AssetNaming::plain(r"\\?\UNC\build\share\artifact.zip"),
        r"\\build\share\artifact.zip"
    );
}

// The prefix exists to express paths the ordinary syntax cannot. Handing back a
// shorter string that no longer opens the file would be worse than the noise it
// removed.
#[test]
fn a_path_that_needs_the_prefix_keeps_it() {
    let deep = format!(r"\\?\C:\{}\notes.txt", "directory".repeat(40));

    assert_eq!(AssetNaming::plain(&deep), deep, "past the plain-path limit");

    // A device path has no plain spelling at all.
    assert_eq!(
        AssetNaming::plain(r"\\?\Volume{b75e2c83-0000-0000-0000-602f00000000}\x"),
        r"\\?\Volume{b75e2c83-0000-0000-0000-602f00000000}\x"
    );
}

#[test]
fn a_path_that_never_had_a_prefix_is_unchanged() {
    for plain in [
        r"C:\Users\charl\notes.txt",
        "/home/charl/notes.txt",
        "notes.txt",
        "",
    ] {
        assert_eq!(AssetNaming::plain(plain), plain);
    }
}

// Two spellings of one file must never produce two ids, or a card built from one
// opens onto nothing. This is why `plain` is for display only and is never fed
// back into `id_for`.
#[test]
fn the_id_is_minted_from_one_spelling_and_only_one() {
    let canonical = r"\\?\C:\Users\charl\uploads\notes.txt";

    assert_ne!(
        AssetNaming::id_for(canonical),
        AssetNaming::id_for(&AssetNaming::plain(canonical)),
        "if these ever match, the two spellings have silently become interchangeable \
         and the guard this test exists for has gone"
    );
}
