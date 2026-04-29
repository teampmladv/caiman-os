#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  Caimán OS — Publish to GitHub
#  Inicializa el repo y hace el primer push a GitHub
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

BRT='\033[38;2;118;255;3m'
GRN='\033[38;2;76;175;80m'
DIM='\033[38;2;74;124;74m'
AMB='\033[38;2;255;179;0m'
RED='\033[38;2;239;83;80m'
NC='\033[0m'

GITHUB_ORG="${1:-teampmladv}"
GITHUB_USER="${2:-teampmladv}"
REPO_NAME="caiman-os"
REMOTE="git@github.com:${GITHUB_ORG}/${REPO_NAME}.git"

echo -e "\n${BRT}🐊 Caimán OS — Publishing to GitHub${NC}"
echo -e "${DIM}   Repo: $REMOTE${NC}\n"

# ── 1. Git init ────────────────────────────────────────────────────────────
if [ ! -d .git ]; then
  echo -e "${GRN}[1/6]${NC} Initializing git repository"
  git init
  git checkout -b main
else
  echo -e "${GRN}[1/6]${NC} Git repo already initialized"
fi

# ── 2. Git config ──────────────────────────────────────────────────────────
echo -e "${GRN}[2/6]${NC} Configuring git"
git config user.name  "Caimán OS"
git config user.email "dev@caiman.io"

# ── 3. Stage all files ─────────────────────────────────────────────────────
echo -e "${GRN}[3/6]${NC} Staging files"
git add -A

# Show summary
echo
echo -e "${DIM}Files to commit:${NC}"
git diff --cached --stat | tail -5
echo -e "${DIM}  ... $(git diff --cached --name-only | wc -l) files total${NC}"
echo

# ── 4. Initial commit ──────────────────────────────────────────────────────
echo -e "${GRN}[4/6]${NC} Creating initial commit"
git commit -m "feat: Caimán OS v0.1.0 — initial commit

🐊 Named after the Cuban crocodile. Built for the cloud.

Stack completo:
- kernel/caiman_net  — módulo kernel C + XDP/eBPF (sin QEMU)
- vmm/               — VMM Rust con KVM directo (/dev/kvm ioctls)
- cni/               — plugin CNI compatible con Calico/Cilium/Flannel
- microseg/          — micro-segmentación XDP zero-trust < 5µs
- drs/               — DRS: σ-balancer + K8s scheduler extender
- storage/           — VSAN distribuido + vVols (iSCSI/NVMe-oF/NFS/FC)
- livemig/           — live migration pre-copy < 200ms blackout
- gpu/               — NVIDIA MIG + vGPU + passthrough
- bts/               — Backup (Restic) + Templates (COW) + Snapshots
- ui/                — Dashboard React: mejor que Rancher y vSphere
- install/           — ISO + script + iDRAC/BMC provisioning

Performance vs vSphere:
  Red:     ~8µs P50 vs ~100µs (XDP zero-copy)
  Microseg: ~5µs vs ~50µs (NSX-T)
  VMM:     ~10MB/VM vs ~250MB (sin QEMU)
  Licencia: \$0/año vs \$4K-\$10K/socket/año"

# ── 5. Add remote ──────────────────────────────────────────────────────────
echo -e "${GRN}[5/6]${NC} Adding remote origin"
git remote add origin "$REMOTE" 2>/dev/null \
  || git remote set-url origin "$REMOTE"

echo -e "${DIM}  Remote: $REMOTE${NC}"

# ── 6. Push ────────────────────────────────────────────────────────────────
echo -e "${GRN}[6/6]${NC} Pushing to GitHub"
echo

# Check if user wants to push
read -rp "$(echo -e "${AMB}¿Hacer push a $REMOTE? [y/N] ${NC}")" ans
if [[ "${ans,,}" != "y" ]]; then
  echo -e "\n${DIM}Push cancelado. Para publicar manualmente:${NC}"
  echo -e "  git remote add origin $REMOTE"
  echo -e "  git push -u origin main"
  exit 0
fi

git push -u origin main

echo
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BRT}  🐊 Caimán OS publicado en GitHub!${NC}"
echo
echo -e "  ${GRN}Repositorio:${NC}  https://github.com/$GITHUB_ORG/$REPO_NAME"
echo -e "  ${GRN}Actions:${NC}      https://github.com/$GITHUB_ORG/$REPO_NAME/actions"
echo -e "  ${GRN}Releases:${NC}     https://github.com/$GITHUB_ORG/$REPO_NAME/releases"
echo
echo -e "  Próximos pasos:"
echo -e "  ${DIM}1. Configurar secrets en GitHub:${NC}"
echo -e "     Settings → Secrets → Actions"
echo -e "     ${DIM}(no se necesitan secretos para el CI básico con GITHUB_TOKEN)${NC}"
echo
echo -e "  ${DIM}2. Publicar primer release:${NC}"
echo -e "     git tag v0.1.0"
echo -e "     git push origin v0.1.0"
echo
echo -e "  ${DIM}3. Habilitar GitHub Pages (docs):${NC}"
echo -e "     Settings → Pages → Branch: main /docs"
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
