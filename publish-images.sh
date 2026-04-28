#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

BRT='\033[38;2;118;255;3m'; GRN='\033[38;2;76;175;80m'
DIM='\033[38;2;74;124;74m'; AMB='\033[38;2;255;179;0m'
RED='\033[38;2;239;83;80m'; NC='\033[0m'

REGISTRY="ghcr.io/teampmladv"
VERSION="${1:-0.1.0}"
PUSH="${2:-}"
RUST_CRATES=(caiman-vmm caiman-api caiman-cni caiman-drs caiman-mcp caiman-bts)
IMAGES=(caiman-vmm caiman-api caiman-cni caiman-drs caiman-mcp caiman-bts caiman-ui)

echo -e "\n${BRT}🐊 Caimán OS — OCI Images v${VERSION}${NC}\n"

# ── Detectar gestor de paquetes ───────────────────────────────────────────
if   command -v dnf  &>/dev/null; then PKG="dnf"
elif command -v yum  &>/dev/null; then PKG="yum"
elif command -v apt-get &>/dev/null; then PKG="apt"
else echo -e "${RED}No se encontró dnf/yum/apt-get${NC}"; exit 1
fi
echo -e "${DIM}  Gestor de paquetes: $PKG${NC}"

# ── Step 1: Instalar dependencias ─────────────────────────────────────────
echo -e "${BRT}━━ Dependencias del sistema${NC}"

case "$PKG" in
  dnf|yum)
    sudo $PKG install -y -q \
        openssl-devel \
        sqlite-devel \
        gcc \
        make \
        clang \
        llvm \
        elfutils-libelf-devel \
        kernel-devel \
        pkgconfig \
        curl \
        2>/dev/null || true
    ;;
  apt)
    sudo apt-get update -qq
    sudo apt-get install -y -qq \
        libssl-dev pkg-config libsqlite3-dev \
        libc6-dev gcc make curl clang llvm libelf-dev \
        2>/dev/null || true
    ;;
esac
echo -e "  ${GRN}✓${NC} Dependencias instaladas"

# ── Step 2: Rust toolchain ────────────────────────────────────────────────
echo -e "${BRT}━━ Rust toolchain${NC}"
if ! command -v cargo &>/dev/null; then
    echo -e "  ${DIM}Instalando Rust…${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --no-modify-path
    source "$HOME/.cargo/env"
fi
source "$HOME/.cargo/env" 2>/dev/null || true
echo -e "  ${GRN}✓${NC} $(cargo --version)"

# ── Step 3: Compilar binarios Rust ────────────────────────────────────────
echo -e "${BRT}━━ Compilando binarios Rust${NC}"
FAILED_RUST=()

for crate in "${RUST_CRATES[@]}"; do
    echo -ne "  $crate… "
    if cargo build --release -p "$crate" 2>/tmp/cargo-err-$crate; then
        if [[ -f "target/release/$crate" ]]; then
            echo -e "${GRN}✓${NC} ($(du -sh target/release/$crate | cut -f1))"
        else
            echo -e "${AMB}compiló pero no se encontró el binario${NC}"
        fi
    else
        echo -e "${RED}✗${NC}"
        echo -e "  ${DIM}Error:${NC}"
        grep "^error" /tmp/cargo-err-$crate | head -5
        FAILED_RUST+=("$crate")
    fi
done

if [[ ${#FAILED_RUST[@]} -gt 0 ]]; then
    echo -e "\n${AMB}  Crates con error: ${FAILED_RUST[*]}${NC}"
    echo -e "  ${DIM}Continuando con los que compilaron…${NC}\n"
fi

# ── Step 4: Node.js + UI ──────────────────────────────────────────────────
echo -e "${BRT}━━ Build UI (React)${NC}"

# Instalar Node.js si no existe
if ! command -v node &>/dev/null; then
    echo -e "  ${DIM}Instalando Node.js 20…${NC}"
    case "$PKG" in
      dnf|yum)
        curl -fsSL https://rpm.nodesource.com/setup_20.x | sudo bash -
        sudo $PKG install -y -q nodejs
        ;;
      apt)
        curl -fsSL https://deb.nodesource.com/setup_20.x | sudo bash -
        sudo apt-get install -y nodejs
        ;;
    esac
fi
echo -e "  ${GRN}✓${NC} $(node --version)"

if [[ -d "ui" ]]; then
    [[ ! -d "ui/node_modules" ]] && { echo -e "  ${DIM}npm install…${NC}"; cd ui && npm ci && cd ..; }
    [[ ! -d "ui/dist" ]]         && { echo -e "  ${DIM}npm run build…${NC}"; cd ui && npm run build && cd ..; }
    echo -e "  ${GRN}✓${NC} UI built ($(du -sh ui/dist | cut -f1))"
fi

# ── Step 5: Docker ────────────────────────────────────────────────────────
echo -e "${BRT}━━ Docker${NC}"
if ! command -v docker &>/dev/null; then
    echo -e "  ${DIM}Instalando Docker…${NC}"
    curl -fsSL https://get.docker.com | sudo sh
    sudo usermod -aG docker "$USER"
    echo -e "  ${AMB}⚠ Docker instalado. Ejecutar 'newgrp docker' o hacer logout/login${NC}"
    echo -e "  ${AMB}  Luego volver a ejecutar: ./publish-images.sh $VERSION $PUSH${NC}"
    exit 0
fi
echo -e "  ${GRN}✓${NC} $(docker --version)"

# Login si vamos a hacer push
if [[ "$PUSH" == "--push" ]]; then
    [[ -z "${GITHUB_TOKEN:-}" ]] && \
        read -srp "$(echo -e "${AMB}GitHub PAT (packages:write): ${NC}")" GITHUB_TOKEN && echo
    echo "$GITHUB_TOKEN" | docker login ghcr.io -u teampmladv --password-stdin
    echo -e "  ${GRN}✓${NC} Login en ghcr.io OK"
fi

# ── Step 6: Docker build + push ───────────────────────────────────────────
echo -e "${BRT}━━ Docker build${NC}"
FAILED=()

for img in "${IMAGES[@]}"; do
    DOCKERFILE="${SCRIPT_DIR}/docker/Dockerfile.${img}"
    [[ ! -f "$DOCKERFILE" ]] && \
        { echo -e "  ${AMB}⚠ $img: sin Dockerfile, saltando${NC}"; continue; }

    # Para imágenes Rust verificar que el binario existe
    if [[ "$img" != "caiman-ui" ]]; then
        [[ ! -f "target/release/$img" ]] && \
            { echo -e "  ${AMB}⚠ $img: binario no compilado, saltando${NC}"; continue; }
    else
        # Para UI verificar que existe el dist
        [[ ! -d "ui/dist" ]] && \
            { echo -e "  ${AMB}⚠ caiman-ui: ui/dist no encontrado, saltando${NC}"; continue; }
    fi

    PUSH_FLAG=""
    [[ "$PUSH" == "--push" ]] && PUSH_FLAG="--push"

    echo -ne "  Building $img… "
    if docker build \
        --platform linux/amd64 \
        --file "$DOCKERFILE" \
        --tag "${REGISTRY}/${img}:${VERSION}" \
        --tag "${REGISTRY}/${img}:latest" \
        . &>/tmp/docker-err-$img; then
        [[ "$PUSH" == "--push" ]] && \
            docker push "${REGISTRY}/${img}:${VERSION}" &>/dev/null && \
            docker push "${REGISTRY}/${img}:latest" &>/dev/null
        echo -e "${GRN}✓${NC}${PUSH:+ (pushed)}"
    else
        echo -e "${RED}✗${NC}"
        tail -5 /tmp/docker-err-$img
        FAILED+=("$img")
    fi
done

# ── Resumen ───────────────────────────────────────────────────────────────
echo
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
[[ ${#FAILED[@]} -gt 0 ]] && echo -e "${RED}  Fallaron: ${FAILED[*]}${NC}"
echo -e "${BRT}  🐊 Completado!${NC}"
[[ "$PUSH" == "--push" ]] && \
    echo -e "  Imágenes en: ${GRN}${REGISTRY}${NC}"
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
