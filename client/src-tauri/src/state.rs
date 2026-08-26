pub struct AppState {
    version: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
