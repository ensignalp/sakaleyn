# Sakaleyn: kaynaktan kullanıcı düzeyinde kurulum (sudo gerekmez).
#   make install        derle + kur (install.sh --bin): servis, sesler, simgeler, ayarlar, GNOME eklentisi
#   make install-tray   GNOME dışında da tepsi simgesini oturum açılışına ekle
#   make uninstall      kaldır (ayarlar ve önbellek kalır)
#   make sounds         ezan kayıtlarını hazırla (../ezan sesleri → assets/sounds/adhan)
#   make dist           sürüm paketleri (statik ikili gerekir: make static)
#   make static         Alpine/musl ile statik ikili (podman ya da docker)
#   make pack           extensions.gnome.org için eklenti zip'i

CARGO ?= cargo
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
UUID := sakaleyn@ensignalp
OCI ?= $(shell command -v podman || command -v docker)

.PHONY: all build test install install-tray uninstall sounds static dist pack clean

all: build

build:
	$(CARGO) build --release --locked

test:
	$(CARGO) test --locked

install: build
	sh install.sh --bin target/release/sakaleyn

install-tray: build
	sh install.sh --bin target/release/sakaleyn --tray

uninstall:
	sh install.sh --uninstall

sounds:
	scripts/prepare-sounds.sh
	scripts/make-alerts.sh

static:
	$(OCI) run --rm -v "$$PWD":/src:Z -w /src docker.io/library/rust:alpine sh -euc \
	  'apk add --no-cache musl-dev gcc make cmake perl >/dev/null && \
	   cargo build --release --locked --target-dir target/alpine && \
	   strip -o /src/target/sakaleyn-static target/alpine/release/sakaleyn'

dist:
	test -x target/sakaleyn-static || { echo "önce: make static"; exit 1; }
	sh scripts/dist-tarball.sh target/sakaleyn-static x86_64 $(VERSION)

pack:
	mkdir -p dist
	rm -f dist/$(UUID).shell-extension.zip
	cd gnome-extension/$(UUID) && zip -qr ../../dist/$(UUID).shell-extension.zip . -x 'schemas/gschemas.compiled'
	@echo dist/$(UUID).shell-extension.zip

clean:
	$(CARGO) clean
	rm -rf dist
