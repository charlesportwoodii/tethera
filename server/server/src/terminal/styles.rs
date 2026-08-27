use tethera_common::protocol::terminal::Style;

/// One frame's style table.
///
/// A linear scan rather than a map: a frame's table is a handful of entries
/// because a terminal row is overwhelmingly one style, so a hash would cost more
/// to build than the scan costs to search.
#[derive(Debug, Default)]
pub struct StyleTable {
    styles: Vec<Style>,
}

impl StyleTable {
    pub fn new() -> Self {
        Self { styles: Vec::new() }
    }

    pub fn intern(&mut self, style: Style) -> u16 {
        if let Some(index) = self.styles.iter().position(|held| *held == style) {
            return u16::try_from(index).unwrap_or(u16::MAX);
        }

        // A table cannot outgrow the index that names it. One frame reaching
        // 65535 distinct styles is a runaway, and reusing the last slot keeps the
        // frame drawable instead of silently indexing something else.
        if self.styles.len() >= usize::from(u16::MAX) {
            return u16::MAX - 1;
        }

        self.styles.push(style);

        u16::try_from(self.styles.len() - 1).unwrap_or(u16::MAX)
    }

    pub fn len(&self) -> usize {
        self.styles.len()
    }

    pub fn into_vec(self) -> Vec<Style> {
        self.styles
    }
}
