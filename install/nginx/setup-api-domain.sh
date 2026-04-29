#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  Configura api.caimanos.com en el servidor KVM
#  Ejecutar en el servidor: sudo ./install/nginx/setup-api-domain.sh
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

BRT='\033[38;2;118;255;3m'; GRN='\033[38;2;76;175;80m'
DIM='\033[38;2;74;124;74m'; RED='\033[38;2;239;83;80m'; NC='\033[0m'

ok()  { echo -e "  ${GRN}✓${NC} $1"; }
die() { echo -e "  ${RED}✗${NC} $1"; exit 1; }

echo -e "\n${BRT}🐊 Configurando api.caimanos.com${NC}\n"

[[ $EUID -eq 0 ]] || die "Ejecutar como root: sudo $0"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOMAIN="api.caimanos.com"

# 1. Instalar nginx
if ! command -v nginx &>/dev/null; then
    command -v dnf &>/dev/null && dnf install -y -q nginx
    command -v apt-get &>/dev/null && apt-get install -y -qq nginx
fi
systemctl enable --now nginx
ok "Nginx running"

# 2. Instalar certbot
if ! command -v certbot &>/dev/null; then
    command -v dnf     &>/dev/null && dnf install -y -q certbot python3-certbot-nginx 2>/dev/null || snap install certbot --classic
    command -v apt-get &>/dev/null && apt-get install -y -qq certbot python3-certbot-nginx
fi
ok "Certbot available"

# 3. Config nginx HTTP temporal (para certbot)
cat > /etc/nginx/conf.d/${DOMAIN}.conf << NGINXEOF
server {
    listen 80;
    server_name ${DOMAIN};
    location / { proxy_pass http://127.0.0.1:8765; }
}
NGINXEOF

nginx -t && systemctl reload nginx
ok "Nginx temporal config"

# 4. Certificado TLS
SERVER_IP=$(curl -4 -sf https://ifconfig.me/ 2>/dev/null || hostname -I | awk '{print $1}')
echo -e "  ${DIM}IP del servidor: $SERVER_IP${NC}"
echo -e "  ${DIM}DNS necesario: $DOMAIN → $SERVER_IP${NC}"
echo -e "  ${DIM}Verificar: dig +short $DOMAIN${NC}\n"

read -p "¿El DNS $DOMAIN ya apunta a $SERVER_IP? [y/N] " ans
if [[ "${ans,,}" == "y" ]]; then
    certbot --nginx -d "$DOMAIN" \
        --non-interactive --agree-tos \
        --email "admin@caimanos.com" \
        --redirect
    ok "Certificado TLS obtenido"
else
    echo -e "  ${DIM}Configura el DNS y ejecuta manualmente:${NC}"
    echo -e "  certbot --nginx -d $DOMAIN --non-interactive --agree-tos --email admin@caimanos.com --redirect"
fi

# 5. Config nginx con SSL + CORS
cp "$SCRIPT_DIR/api.caimanos.com.conf" /etc/nginx/conf.d/${DOMAIN}.conf
rm -f /etc/nginx/conf.d/${DOMAIN}.conf 2>/dev/null || true
cp "$SCRIPT_DIR/api.caimanos.com.conf" /etc/nginx/sites-available/${DOMAIN} 2>/dev/null || \
    cp "$SCRIPT_DIR/api.caimanos.com.conf" /etc/nginx/conf.d/${DOMAIN}.conf
ln -sf /etc/nginx/sites-available/${DOMAIN} /etc/nginx/sites-enabled/ 2>/dev/null || true

nginx -t && systemctl reload nginx
ok "Nginx con SSL + CORS configurado"

# 6. Firewall
command -v firewall-cmd &>/dev/null && firewall-cmd --permanent --add-service=http --add-service=https &>/dev/null && firewall-cmd --reload &>/dev/null || true
command -v ufw &>/dev/null && ufw allow 'Nginx Full' &>/dev/null || true
ok "Firewall: HTTP + HTTPS"

# 7. Arrancar caiman stack
cd /root/caiman-os 2>/dev/null || cd /opt/caiman 2>/dev/null || true
if [[ -f "docker-compose.yml" ]]; then
    docker compose up -d 2>/dev/null || docker-compose up -d
    ok "Caimán stack running"
fi

echo
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BRT}  🐊 api.caimanos.com configurado!${NC}"
echo
echo -e "  ${GRN}API:${NC}    https://api.caimanos.com/health"
echo -e "  ${GRN}VMs:${NC}    https://api.caimanos.com/api/vms"
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
