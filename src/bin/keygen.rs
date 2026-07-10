use rand::RngCore;

fn main() {
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();

    let pub_hex = hex::encode(verifying_key.to_bytes());
    let priv_hex = hex::encode(signing_key.to_bytes());

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           Synapsis Ed25519 Keypair Generator               ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Public key (embed in binary via SYNAPSIS_PUBKEY env var):");
    println!("{}", pub_hex);
    println!();
    println!("Private key (KEEP SAFE! Used to sign licenses):");
    println!("{}", priv_hex);
    println!();
    println!("To build with license verification:");
    println!("  SYNAPSIS_PUBKEY={} cargo build --release", pub_hex);
    println!();
    println!("To sign a license file:");
    println!("  create a LicenseData JSON, then:");
    println!("  echo '<priv-key>' | synapsis-keygen-sign <license.json>");
}
