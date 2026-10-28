#!/usr/bin/env bash
# Gera os instaladores do FeatherDocs em pacote/saida/, com os nomes fixos que o
# launcher procura na release (releases/latest/download/<nome>):
#   featherdocs-linux-x86_64.tar.gz   Linux portátil (instalar.sh na pasta do usuário)
#   featherdocs-linux-amd64.deb       Debian, Ubuntu, Mint…
#   FeatherDocs-windows-x64.exe       Windows 10/11 (instalador por usuário, sem administrador)
#   SHA256SUMS
#
# Uso: pacote/gerar.sh [linux|windows ...]   (sem argumentos = todos)
# Precisa de: cargo, dpkg-deb, objdump; podman ou docker para o Linux (sem eles compila aqui,
# e o .deb só instala em distros com a glibc desta máquina); para Windows, o alvo x86_64-pc-windows-gnu
# (rustup target add x86_64-pc-windows-gnu), mingw-w64 e makensis.
set -euo pipefail

pacote="$(cd "$(dirname "$0")" && pwd)"
raiz="$(dirname "$pacote")"
gui="$raiz/crates/featherdocs-pelicano"
versao="$(sed -n 's/^version = "\(.*\)"/\1/p' "$gui/Cargo.toml" | head -1)"
saida="$pacote/saida"
alvos=("$@")
[[ ${#alvos[@]} -eq 0 ]] && alvos=(linux windows)
rm -rf "$saida" && mkdir -p "$saida"
echo "FeatherDocs $versao → $saida"

# O binário pede a glibc de onde foi compilado: num Debian 12 (2.36) ele roda no Ubuntu 22.04+,
# Debian 12+ e Mint 21+. A imagem é a mesma do FeatherNotes (só Rust sobre o Debian 12).
compilar_linux() {
  local motor; motor="$(command -v podman || command -v docker || true)"
  if [[ -z $motor ]]; then
    echo "Sem podman nem docker: compilando aqui (o .deb vai pedir a glibc desta máquina)." >&2
    cargo build --release --manifest-path "$gui/Cargo.toml"
    bin="$gui/target/release/featherdocs-pelicano"
    return
  fi
  if ! "$motor" image inspect feathernotes-debian12 >/dev/null 2>&1; then
    "$motor" build -t feathernotes-debian12 -f - "$pacote" <<'IMG'
FROM docker.io/library/debian:12
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev curl ca-certificates git && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH=/root/.cargo/bin:$PATH
IMG
  fi
  "$motor" run --rm --memory 8g -v "$raiz":/src:z -v feathernotes-registro:/root/.cargo/registry -e CARGO_TARGET_DIR=/src/target/debian12 -w /src \
    feathernotes-debian12 cargo build --release -j 6 --manifest-path crates/featherdocs-pelicano/Cargo.toml
  bin="$raiz/target/debian12/release/featherdocs-pelicano"
}

linux() {
  local bin; compilar_linux
  strip "$bin" 2>/dev/null || true
  local glibc; glibc="$(objdump -T "$bin" | grep -o 'GLIBC_2\.[0-9]*' | sort -V | tail -1 | cut -d_ -f2)"
  echo "glibc mínima: $glibc"

  # .deb
  local d; d="$(mktemp -d)"
  install -Dm755 "$bin" "$d/usr/bin/featherdocs"
  install -Dm644 "$pacote/featherdocs.desktop" "$d/usr/share/applications/featherdocs.desktop"
  install -Dm644 "$pacote/featherdocs-mime.xml" "$d/usr/share/mime/packages/featherdocs.xml"
  for t in 16 24 32 48 64 128 256 512; do
    install -Dm644 "$pacote/icones/featherdocs-$t.png" "$d/usr/share/icons/hicolor/${t}x${t}/apps/featherdocs.png"
    install -Dm644 "$pacote/icones/featherdocs-$t.png" "$d/usr/share/icons/hicolor/${t}x${t}/mimetypes/application-x-featherdocs.png"
  done
  install -Dm644 "$pacote/icones/featherdocs.svg" "$d/usr/share/icons/hicolor/scalable/apps/featherdocs.svg"
  install -Dm644 "$raiz/LICENSE" "$d/usr/share/doc/featherdocs/copyright"
  local tamanho; tamanho="$(du -sk "$d/usr" | cut -f1)"
  mkdir -p "$d/DEBIAN"
  cat > "$d/DEBIAN/control" <<EOF
Package: featherdocs
Version: $versao
Section: editors
Priority: optional
Architecture: amd64
Installed-Size: $tamanho
Depends: libc6 (>= $glibc), libxkbcommon0, libwayland-client0 | libx11-6, libfontconfig1
Recommends: libvulkan1 | libgl1
Maintainer: FeatherWorks <https://github.com/feather-works>
Homepage: https://github.com/feather-works/featherdocs
Description: Editor de documentos da suíte FeatherWorks
 Editor de documentos em Rust com interface nativa: .docx, LaTeX e
 Markdown, com o histórico de versões dentro do arquivo. Roda inteiro na
 máquina: sem conta, sem login, sem telemetria.
EOF
  cat > "$d/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v update-mime-database >/dev/null && update-mime-database /usr/share/mime || true
command -v update-desktop-database >/dev/null && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
EOF
  cp "$d/DEBIAN/postinst" "$d/DEBIAN/postrm"
  chmod 755 "$d/DEBIAN/postinst" "$d/DEBIAN/postrm"
  dpkg-deb --root-owner-group --build "$d" "$saida/featherdocs-linux-amd64.deb" >/dev/null
  rm -rf "$d"

  # .tar.gz portátil: "<app>-<versão>/instalar.sh", o mesmo que o launcher roda.
  local t; t="$(mktemp -d)/featherdocs-$versao"
  mkdir -p "$t"
  cp "$bin" "$t/featherdocs"
  cp "$pacote/featherdocs.desktop" "$pacote/featherdocs-mime.xml" "$raiz/LICENSE" "$t/"
  cp -r "$pacote/icones" "$t/"
  cat > "$t/instalar.sh" <<'EOF'
#!/bin/sh
# Instala o FeatherDocs só para este usuário (~/.local), sem administrador.
set -e
cd "$(dirname "$0")"
mkdir -p ~/.local/bin ~/.local/share/applications ~/.local/share/mime/packages
install -m755 featherdocs ~/.local/bin/featherdocs
sed "s#^Exec=featherdocs#Exec=$HOME/.local/bin/featherdocs#" featherdocs.desktop > ~/.local/share/applications/featherdocs.desktop
cp featherdocs-mime.xml ~/.local/share/mime/packages/featherdocs.xml
for t in 16 24 32 48 64 128 256 512; do
  mkdir -p ~/.local/share/icons/hicolor/${t}x${t}/apps
  cp icones/featherdocs-$t.png ~/.local/share/icons/hicolor/${t}x${t}/apps/featherdocs.png
done
update-mime-database ~/.local/share/mime 2>/dev/null || true
update-desktop-database ~/.local/share/applications 2>/dev/null || true
echo "FeatherDocs instalado. Procure por \"FeatherDocs\" no menu de aplicativos."
EOF
  chmod 755 "$t/instalar.sh"
  tar -C "$(dirname "$t")" -czf "$saida/featherdocs-linux-x86_64.tar.gz" "featherdocs-$versao"
  rm -rf "$(dirname "$t")"
}

windows() {
  cargo build --release --target x86_64-pc-windows-gnu --manifest-path "$gui/Cargo.toml"
  local exe="$gui/target/x86_64-pc-windows-gnu/release/featherdocs-pelicano.exe"
  x86_64-w64-mingw32-strip "$exe" 2>/dev/null || true
  (cd "$pacote" && makensis -V2 -DVERSAO="$versao" -DEXE="$exe" -DSAIDA="$saida/FeatherDocs-windows-x64.exe" featherdocs.nsi)
}

for a in "${alvos[@]}"; do
  echo "== $a"
  "$a"
done
(cd "$saida" && sha256sum -- * | grep -v SHA256SUMS > SHA256SUMS)
ls -lh "$saida"
