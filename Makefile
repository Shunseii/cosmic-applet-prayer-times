# cosmic-applet-prayer-times
#
# Usage:
#   make build              # release build (run as your user)
#   sudo make install       # install system-wide (default PREFIX=/usr)
#   sudo make uninstall
#
# Overridable:
#   make install PREFIX=/usr/local
#   make build  CARGO=$HOME/.cargo/bin/cargo   # if cargo isn't on PATH
#   make install DESTDIR=/tmp/pkg              # staged/packaging install

NAME  := cosmic-applet-prayer-times
APPID := io.github.shunseii.CosmicAppletPrayerTimes

CARGO   ?= cargo
PREFIX  ?= /usr
DESTDIR ?=

TARGET_DIR ?= target
BIN_SRC    := $(TARGET_DIR)/release/$(NAME)

BIN_DST      := $(DESTDIR)$(PREFIX)/bin/$(NAME)
DESKTOP_DST  := $(DESTDIR)$(PREFIX)/share/applications/$(APPID).desktop
ICON_DST     := $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/$(APPID).svg
METAINFO_DST := $(DESTDIR)$(PREFIX)/share/metainfo/$(APPID).metainfo.xml

.PHONY: all build run check clean install uninstall

all: build

build:
	$(CARGO) build --release

run:
	env RUST_BACKTRACE=full $(CARGO) run --release

check:
	$(CARGO) clippy --all-features -- -W clippy::pedantic

clean:
	$(CARGO) clean

# Installs the already-built release binary. Run `make build` first (as your
# user); this target does not invoke cargo, so `sudo make install` won't rebuild
# as root.
install: $(BIN_SRC)
	install -Dm0755 $(BIN_SRC) $(BIN_DST)
	install -Dm0644 resources/app.desktop $(DESKTOP_DST)
	install -Dm0644 resources/app.metainfo.xml $(METAINFO_DST)
	install -Dm0644 resources/icon.svg $(ICON_DST)
	-update-desktop-database $(DESTDIR)$(PREFIX)/share/applications 2>/dev/null || true

$(BIN_SRC):
	@echo "error: $(BIN_SRC) not found — run 'make build' first (as your user)." >&2
	@exit 1

uninstall:
	rm -f $(BIN_DST) $(DESKTOP_DST) $(ICON_DST) $(METAINFO_DST)
	-update-desktop-database $(DESTDIR)$(PREFIX)/share/applications 2>/dev/null || true
