# tethera-relay

An Iroh relay that admits only callers holding a shared secret. Two peers that cannot
reach each other directly relay through it; everyone else is refused at the relay
handshake.

The secret gates the relay listener only. The QUIC address-discovery listener on
`quic_bind` is run by `iroh-relay` without any `AccessControl`, so it answers anyone who
reaches it. It relays nothing — it reports the address a caller was seen at — but the
README should not be read as "the secret gates every port".

## The secret is shared, and it must match everywhere

`secret` in `relay.toml` is the same string every Tethera server and every client sends:

```rust
RelayConfig::new(url, None).with_auth_token(secret)
```

`with_auth_token` takes `impl Into<String>`, not an `Option`. `transport/src/endpoint/mod.rs`
is the reference call site.

A caller that sends a different token, or no token, is denied. There is no second way in,
so a mismatch looks like a relay that never works rather than one that works intermittently.
Use a long random string and treat it as a credential.

Never put the secret in a URL. `iroh-relay` reads the token from an `Authorization: Bearer`
header, and falls back to a `?token=` query parameter when no header is present. Our access
control accepts whatever that returns, so the query-parameter form works — and lands the
credential in the access log of every proxy and load balancer between the caller and the
relay. Send the header.

Leading and trailing whitespace is trimmed when the file is read, so `secret = "  abc  "`
is the secret `abc` on both sides.

## Certificates must be supplied

DNS-01 issuance against Cloudflare is not implemented. `iroh-relay` ships ACME through
`tokio-rustls-acme`, which speaks TLS-ALPN-01 only, so the Cloudflare path has to issue
outside the relay and hand it the result. Until that exists, obtain the certificate
yourself and point the relay at it:

```toml
tls_cert_path = "/etc/tethera/fullchain.pem"
tls_key_path  = "/etc/tethera/privkey.pem"
```

Set both or neither. One without the other is a startup error rather than a silent fall
back to plain HTTP. With neither set the relay serves plain HTTP on `http_bind`, which is
fine behind a TLS terminator and fine for local work.

An `[acme]` block is accepted but not yet acted on: with no certificate paths configured it
reports that issuance for the named domains is not implemented.

## Running

```sh
cargo run -p tethera-relay --bin tethera-relay -- run --config relay/config.example.toml
```

Or with the container files here, after copying `config.example.toml` to `relay/relay.toml`
and setting a real secret:

```sh
docker compose -f relay/compose.yaml up -d
```

## Configuration keys

| Key | Default | Meaning |
| --- | --- | --- |
| `http_bind` | `0.0.0.0:8080` | Plain HTTP listener |
| `https_bind` | `0.0.0.0:443` | HTTPS listener, used only when TLS is configured |
| `quic_bind` | unset | QUIC address discovery listener; requires TLS |
| `secret` | required | The shared token every server and client must send |
| `tls_cert_path` | unset | PEM certificate chain |
| `tls_key_path` | unset | PEM private key |
| `[acme]` | unset | DNS-01 settings, reserved for issuance that is not implemented |
