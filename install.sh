set -euo pipefail
cargo build --release
sudo cp target/release/phantomlink /bin/phantomlink