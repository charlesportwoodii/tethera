use crate::config::ApplicationConfig;
use crate::machine::{Machine, MachineAddress};
use qrcode::render::unicode;
use qrcode::QrCode;
use tethera_common::structs::pairing::PairingOffer;

/// The address half of pairing: how a phone that has never met this machine
/// reaches it.
///
/// Carries no secret and stays valid indefinitely. What stops a stranger is the
/// pairing window, not this.
pub struct Offer;

impl Offer {
    pub fn build(
        config: &ApplicationConfig,
        endpoint_id: &str,
        now: i64,
    ) -> PairingOffer {
        // Absent when the server is not running. Direct addresses are an
        // optimisation - an endpoint id alone is dialable through discovery -
        // so a stale record costs a slower first connection, never a failed
        // one, and publishing addresses nothing is listening on would cost the
        // opposite.
        let published = MachineAddress::read(config).filter(|record| {
            record.is_fresh(now) && record.endpoint_id == endpoint_id
        });

        let relay = config
            .relay_url
            .clone()
            .or_else(|| published.as_ref().and_then(|record| record.relay.clone()));

        PairingOffer::new(
            Machine::server_id(endpoint_id).as_str().to_string(),
            Some(endpoint_id.to_string()),
            relay,
            published
                .map(|record| record.direct_addrs)
                .unwrap_or_default(),
            Some(Machine::label(config, endpoint_id)),
        )
    }

    // Half blocks rather than the image renderer, so the offer is readable
    // over the same SSH session the operator already has open. Dark and light
    // are swapped because a terminal draws light glyphs on a dark ground and
    // a scanner needs the quiet zone to be the lighter of the two.
    pub fn qr(uri: &str) -> anyhow::Result<String> {
        Ok(QrCode::new(uri.as_bytes())?
            .render::<unicode::Dense1x2>()
            .dark_color(unicode::Dense1x2::Light)
            .light_color(unicode::Dense1x2::Dark)
            .build())
    }
}
