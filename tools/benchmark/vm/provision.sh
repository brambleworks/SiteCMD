#!/bin/bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

test "$(uname -s)" = Linux
test "$(uname -m)" = aarch64
test "$(id -u)" = 0

if test -f /opt/sitecmd-benchmark/provisioned; then
  systemctl daemon-reload
  systemctl restart sitecmd-benchmark-firewall.service
  /opt/sitecmd-benchmark/verify.sh
  exit
fi

apt-get update -qq
apt-get install -y --no-install-recommends \
  build-essential ca-certificates curl file git jq pkg-config xz-utils \
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev \
  librsvg2-dev xvfb xauth dbus-x11 x11-utils fonts-dejavu-core at-spi2-core \
  python3 python3-gi gir1.2-webkit2-4.1 sqlite3 nftables util-linux

for benchmark_user in runner grader sitecmd builder; do
  if ! id "$benchmark_user" >/dev/null 2>&1; then
    useradd --create-home --shell /bin/bash "$benchmark_user"
  fi
  chmod 700 "/home/$benchmark_user"
done
chmod 700 /home/benchadmin
install -d -m 755 /srv/sitecmd-benchmark
install -d -m 700 -o grader -g grader /srv/sitecmd-benchmark/graders
install -d -m 700 -o runner -g runner /srv/sitecmd-benchmark/workspaces
install -d -m 700 -o sitecmd -g sitecmd /srv/sitecmd-benchmark/app-data
install -d -m 700 -o builder -g builder /srv/sitecmd-benchmark/build
install -m 600 -o grader -g grader /opt/sitecmd-benchmark/grader-canary /srv/sitecmd-benchmark/graders/canary

benchmark_lock=/opt/sitecmd-benchmark/runtime-lock.json
benchmark_download_dir=$(mktemp -d /tmp/sitecmd-provision.XXXXXXXX)
benchmark_node_version=$(jq -r '.node.version' "$benchmark_lock")
if ! test -x "/opt/sitecmd-node-$benchmark_node_version/bin/node"; then
  curl --fail --silent --show-error --location --retry 3 --max-time 180 \
    "$(jq -r '.node.url' "$benchmark_lock")" --output "$benchmark_download_dir/node.tar.xz"
  test "$(sha256sum "$benchmark_download_dir/node.tar.xz" | cut -d ' ' -f 1)" = "$(jq -r '.node.sha256' "$benchmark_lock")"
  install -d -m 755 "/opt/sitecmd-node-$benchmark_node_version"
  tar -xJf "$benchmark_download_dir/node.tar.xz" --strip-components=1 -C "/opt/sitecmd-node-$benchmark_node_version"
fi
for benchmark_binary in node npm npx; do
  ln -sfn "/opt/sitecmd-node-$benchmark_node_version/bin/$benchmark_binary" "/usr/local/bin/$benchmark_binary"
done
benchmark_pnpm_version=$(jq -r '.pnpm' "$benchmark_lock")
if ! test "$(pnpm --version 2>/dev/null || true)" = "$benchmark_pnpm_version"; then
  npm install --global --ignore-scripts --prefix /usr/local "pnpm@$benchmark_pnpm_version"
fi

benchmark_rust_version=$(jq -r '.rust' "$benchmark_lock")
benchmark_toolchain="/opt/sitecmd-rust/rustup/toolchains/$benchmark_rust_version-aarch64-unknown-linux-gnu"
if ! test -x "$benchmark_toolchain/bin/rustc"; then
  curl --fail --silent --show-error --location --retry 3 --max-time 180 \
    "$(jq -r '.rustup.url' "$benchmark_lock")" --output "$benchmark_download_dir/rustup-init"
  test "$(sha256sum "$benchmark_download_dir/rustup-init" | cut -d ' ' -f 1)" = "$(jq -r '.rustup.sha256' "$benchmark_lock")"
  chmod 700 "$benchmark_download_dir/rustup-init"
  CARGO_HOME=/opt/sitecmd-rust/cargo RUSTUP_HOME=/opt/sitecmd-rust/rustup \
    "$benchmark_download_dir/rustup-init" -y --no-modify-path --profile minimal --default-toolchain "$benchmark_rust_version"
fi
for benchmark_binary in cargo rustc rustdoc; do
  ln -sfn "$benchmark_toolchain/bin/$benchmark_binary" "/usr/local/bin/$benchmark_binary"
done

chmod 755 /opt/sitecmd-benchmark/verify.sh
systemctl daemon-reload
systemctl enable --now sitecmd-benchmark-firewall.service
systemctl mask --now apt-daily.timer apt-daily-upgrade.timer unattended-upgrades.service
dpkg-query -W > /opt/sitecmd-benchmark/installed-packages.txt
/opt/sitecmd-benchmark/verify.sh
touch /opt/sitecmd-benchmark/provisioned
