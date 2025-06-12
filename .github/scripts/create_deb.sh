set -euo pipefail
version=$(grep version Cargo.toml | head -1 | awk '{print $3}' | tr -d '"')
# create debian dir
mkdir -p debian/usr/bin debian/DEBIAN
cp target/release/phantomlink debian/usr/bin/
cat > debian/DEBIAN/control <<- EOF
Package: phantomlink
Version: $version
Section: net
Priority: optional
Architecture: amd64
Maintainer: Robin Ohs <me@robinohs.dev>
Uploaders: Gregory Stock <g.stock@cs.uni-saarland.de>, Andreas Schmidt <contact@netzdoktor.eu>
Homepage: https://depend.cs.uni-saarland.de
Repository: https://github.com/robinohs/phantomlink
Description: phantomlink looks like a multi-hop Internet path, but emulates a virtual end-to-end link 
EOF

dpkg-deb -Zgzip --build debian phantomlink_${version}_amd64.deb
