use crate::config::ApplicationConfig;
use anyhow::Context;
use std::io::BufRead;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// The device to approve
    pub id: String,

    /// The pairing code the device was issued. Omit it to read the code from
    /// stdin, which keeps the secret out of the shell history and out of ps.
    #[clap(long)]
    pub code: Option<String>,
}

impl Config {
    pub async fn run(&self, _config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let _code = self.resolve_code()?;

        println!("will approve device {} against its pairing code", self.id);

        Ok(())
    }

    // A secret handed to argv is readable by any process that can call ps and
    // is written to the shell history verbatim. The flag stays for scripting,
    // where the caller has already decided how the value reaches the process.
    fn resolve_code(&self) -> anyhow::Result<String> {
        if let Some(code) = &self.code {
            return Ok(code.clone());
        }

        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .context("reading the pairing code from stdin")?;

        let code = line.trim().to_string();

        if code.is_empty() {
            anyhow::bail!("no pairing code supplied on stdin; pass --code to supply it as an argument");
        }

        Ok(code)
    }
}
