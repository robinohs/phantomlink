set -euo pipefail
version=$(grep version Cargo.toml | head -1 | awk '{print $3}' | tr -d '"')
# create debian dir
rpm_path=phantomlink-$version

mkdir -p $rpm_path $rpm_path/usr/bin
chmod +x phantomlink
cp phantomlink $rpm_path/usr/bin/
cat > $rpm_path/phantomlink.spec <<- EOF
Name: phantomlink
Version: $version
Release: 1
Summary: phantomlink looks like a multi-hop Internet path, but emulates a virtual end-to-end link 
License: Apache-2.0 or MIT
Group: Applications/Emulators

%define _rpmdir .
%define _rpmfilename %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm
%define _unpackaged_files_terminate_build 0

%description

%files
"/usr/bin/phantomlink"

EOF
rpmbuild --target=x86_64 --buildroot=$PWD/$rpm_path -bb $rpm_path/phantomlink.spec