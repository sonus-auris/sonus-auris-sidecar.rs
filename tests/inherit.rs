#![forbid(unsafe_code)]

use ores_otel_sidecar::{health, SidecarIdentity};

#[test]
fn inherits_shared_health() {
    let identity = SidecarIdentity::new("sonus-auris-sidecar", "SONUS_AURIS_SIDECAR_BIND");
    let payload = health::current(identity, None);
    assert!(payload.ok);
    assert_eq!(payload.service, "sonus-auris-sidecar");
}
