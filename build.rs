use std::path::PathBuf;

fn main() {
    let protos: Vec<PathBuf> = glob::glob("proto/**/*.proto")
        .expect("Error parsing proto glob")
        .map(|result| result.expect("Proto file cannot be read").to_path_buf())
        .collect();

    tonic_build::configure()
        .compile(protos.as_slice(), &["proto".into()])
        .expect("Error compiling protos")
}