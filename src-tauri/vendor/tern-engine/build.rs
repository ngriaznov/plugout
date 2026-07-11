//! Fetches the packed int8 model into OUT_DIR at build time. The model is a
//! release artifact (not in the upstream git repo), so the vendored crate
//! downloads it once and verifies a pinned hash; subsequent builds are offline.
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{env, fs, path::PathBuf};

const MODEL_URL: &str =
    "https://huggingface.co/wenshutang/ternlight/resolve/main/model-embedding-int8.bin";
const MODEL_SHA256: &str = "5b693903bfc57b1699ca2c3f1d87332801f53a89885867eb63f3f8fc6ccce399";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("model.bin");
    if let Ok(existing) = fs::read(&out) {
        if hex(&Sha256::digest(&existing)) == MODEL_SHA256 {
            return;
        }
    }
    let resp = ureq::get(MODEL_URL).call().expect("download tern model.bin");
    let mut body = Vec::new();
    resp.into_reader()
        .read_to_end(&mut body)
        .expect("read tern model.bin body");
    assert_eq!(
        hex(&Sha256::digest(&body)),
        MODEL_SHA256,
        "model.bin hash mismatch — refusing to embed an unverified model"
    );
    fs::write(&out, body).expect("write model.bin to OUT_DIR");
}
