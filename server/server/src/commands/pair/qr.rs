use crate::config::ApplicationConfig;
use crate::identity::Identity;
use qrcode::render::unicode;
use qrcode::QrCode;
use std::sync::Arc;
use tethera_common::structs::pairing::PairingOffer;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let secret_key = Identity::load_or_create(&config.identity_path())?;
        let endpoint_id = secret_key.public().to_string();

        let offer = PairingOffer::new(endpoint_id.clone(), Some(endpoint_id), Vec::new());
        let uri = offer.to_uri();

        println!("{}", Self::render(&uri)?);
        println!("{uri}");

        Ok(())
    }

    // Half blocks rather than the image renderer, so the offer is readable
    // over the same SSH session the operator already has open. Dark and light
    // are swapped because a terminal draws light glyphs on a dark ground and
    // a scanner needs the quiet zone to be the lighter of the two.
    fn render(uri: &str) -> anyhow::Result<String> {
        Ok(QrCode::new(uri.as_bytes())?
            .render::<unicode::Dense1x2>()
            .dark_color(unicode::Dense1x2::Light)
            .light_color(unicode::Dense1x2::Dark)
            .build())
    }
}
