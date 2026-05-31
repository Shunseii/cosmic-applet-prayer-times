name := 'cosmic-applet-prayer-times'
appid := 'io.github.shunseii.CosmicAppletPrayerTimes'

rootdir := ''
prefix := '/usr'

base-dir := absolute_path(clean(rootdir / prefix))
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
appdata-dst := base-dir / 'share' / 'metainfo' / appid + '.metainfo.xml'
bin-dst := base-dir / 'bin' / name
desktop-dst := base-dir / 'share' / 'applications' / appid + '.desktop'
icon-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg'

# Default recipe runs a release build
default: build-release

clean:
    cargo clean

# Debug build
build-debug *args:
    cargo build {{args}}

# Release build
build-release *args: (build-debug '--release' args)

# Clippy check
check *args:
    cargo clippy --all-features {{args}} -- -W clippy::pedantic

# Run for testing
run *args:
    env RUST_BACKTRACE=full cargo run --release {{args}}

# Install built files
install:
    install -Dm0755 {{ cargo-target-dir / 'release' / name }} {{bin-dst}}
    install -Dm0644 resources/app.desktop {{desktop-dst}}
    install -Dm0644 resources/app.metainfo.xml {{appdata-dst}}
    install -Dm0644 resources/icon.svg {{icon-dst}}

uninstall:
    rm {{bin-dst}} {{desktop-dst}} {{icon-dst}} {{appdata-dst}}
