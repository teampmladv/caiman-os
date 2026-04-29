#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  caimanos.com — deploy website to production
#  Run from the caiman-os repo root:
#    sudo ./install/scripts/deploy-website.sh
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

BRT='\033[38;2;118;255;3m'; GRN='\033[38;2;76;175;80m'
DIM='\033[38;2;74;124;74m'; AMB='\033[38;2;255;179;0m'
RED='\033[38;2;239;83;80m'; NC='\033[0m'

DOMAIN="caimanos.com"
WEB_DIR="/opt/caiman/website"
NGINX_CONF="/etc/nginx/sites-available/${DOMAIN}"

step() { echo -e "\n${BRT}━━ $1${NC}"; }
ok()   { echo -e "  ${GRN}✓${NC} $1"; }
warn() { echo -e "  ${AMB}⚠${NC} $1"; }

echo -e "\n${BRT}🐊 Deploying caimanos.com${NC}\n"

[[ $EUID -eq 0 ]] || { echo "Run as root: sudo $0"; exit 1; }

# ── 1. Install Nginx ──────────────────────────────────────────────────────
step "Nginx"
if ! command -v nginx &>/dev/null; then
    if command -v dnf &>/dev/null; then
        dnf install -y -q nginx
    elif command -v apt-get &>/dev/null; then
        apt-get install -y -qq nginx
    fi
fi
ok "Nginx $(nginx -v 2>&1 | grep -o '[0-9.]*$')"

# ── 2. Install Certbot ────────────────────────────────────────────────────
step "TLS Certificate"
if ! command -v certbot &>/dev/null; then
    if command -v dnf &>/dev/null; then
        dnf install -y -q certbot python3-certbot-nginx 2>/dev/null || \
            snap install certbot --classic
    else
        apt-get install -y -qq certbot python3-certbot-nginx
    fi
fi
ok "Certbot available"

# ── 3. Deploy website files ───────────────────────────────────────────────
step "Website files"
mkdir -p "$WEB_DIR"

# Copy from repo or download
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEBSITE_SRC="$SCRIPT_DIR/../../website"

if [[ -d "$WEBSITE_SRC" ]]; then
    cp -r "$WEBSITE_SRC/"* "$WEB_DIR/"
    ok "Website copied from repo"
elif [[ -f "website/index.html" ]]; then
    cp -r website/* "$WEB_DIR/"
    ok "Website copied"
else
    # Download from GitHub releases
    curl -fsSL "https://github.com/teampmladv/caiman-os/releases/latest/download/website.tar.gz" \
        | tar -xz -C "$WEB_DIR/" 2>/dev/null || \
    warn "Website files not found — create $WEB_DIR/index.html manually"
fi

chmod -R 755 "$WEB_DIR"
ok "Files at $WEB_DIR"

# ── 4. Nginx config ───────────────────────────────────────────────────────
step "Nginx configuration"

# First deploy with plain HTTP config (needed for certbot)
cat > "$NGINX_CONF" << NGINXEOF
server {
    listen 80;
    server_name ${DOMAIN} www.${DOMAIN};
    root ${WEB_DIR};
    index index.html;

    location / { try_files \$uri \$uri/ /index.html; }

    location /api/ {
        proxy_pass         http://127.0.0.1:8765;
        proxy_http_version 1.1;
        proxy_set_header   Host \$host;
        proxy_set_header   X-Real-IP \$remote_addr;
        add_header Access-Control-Allow-Origin "*";
        add_header Access-Control-Allow-Methods "GET, POST, DELETE, OPTIONS";
        add_header Access-Control-Allow-Headers "Content-Type";
        if (\$request_method = OPTIONS) { return 204; }
    }

    location = /install.sh {
        alias /opt/caiman/caiman-os/install/scripts/install.sh;
        add_header Content-Type text/plain;
    }

    location /health { proxy_pass http://127.0.0.1:8765/health; }
}
NGINXEOF

ln -sf "$NGINX_CONF" /etc/nginx/sites-enabled/ 2>/dev/null || true

# Remove default site
rm -f /etc/nginx/sites-enabled/default 2>/dev/null || true

nginx -t && systemctl reload nginx
ok "Nginx reloaded"

# ── 5. Get TLS certificate ────────────────────────────────────────────────
step "TLS certificate"

SERVER_IP=$(curl -4 -sf https://ifconfig.me/ || hostname -I | awk '{print $1}')
echo -e "  ${DIM}Server IP: $SERVER_IP${NC}"
echo -e "  ${DIM}Make sure DNS: $DOMAIN → $SERVER_IP${NC}"

if [[ ! -f "/etc/letsencrypt/live/${DOMAIN}/fullchain.pem" ]]; then
    echo -ne "  ${DIM}Getting certificate from Let's Encrypt…${NC} "
    if certbot --nginx -d "$DOMAIN" -d "www.${DOMAIN}" \
            --non-interactive \
            --agree-tos \
            --email "admin@${DOMAIN}" \
            --redirect 2>/dev/null; then
        echo -e "${GRN}✓${NC}"
    else
        warn "Certbot failed — running HTTP only"
        warn "Fix DNS then run: certbot --nginx -d ${DOMAIN} -d www.${DOMAIN}"
    fi
else
    ok "Certificate already exists ($(certbot certificates 2>/dev/null | grep 'Expiry Date' | head -1 | awk '{print $3,$4}'))"
fi

# ── 6. Auto-renew ─────────────────────────────────────────────────────────
step "Auto-renew"
(crontab -l 2>/dev/null; echo "0 3 * * * certbot renew --quiet && systemctl reload nginx") \
    | sort -u | crontab -
ok "Cron job added for certificate renewal"

# ── 7. Firewall ───────────────────────────────────────────────────────────
step "Firewall"
if command -v firewall-cmd &>/dev/null; then
    firewall-cmd --permanent --add-service=http --add-service=https &>/dev/null || true
    firewall-cmd --reload &>/dev/null || true
    ok "firewalld: HTTP + HTTPS allowed"
elif command -v ufw &>/dev/null; then
    ufw allow 'Nginx Full' &>/dev/null || true
    ok "UFW: Nginx Full allowed"
fi

# ── Done ─────────────────────────────────────────────────────────────────
echo
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BRT}  🐊 caimanos.com deployed!${NC}"
echo
echo -e "  ${GRN}Website:${NC}    https://${DOMAIN}"
echo -e "  ${GRN}API:${NC}        https://${DOMAIN}/api/vms"
echo -e "  ${GRN}Install:${NC}    curl -fsSL https://${DOMAIN}/install.sh | sudo bash"
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
