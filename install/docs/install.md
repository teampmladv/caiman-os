# Caimán OS — Guía de instalación

> Born in Cuba. Built for the cloud.

---

## Requisitos de hardware

### Nodo mínimo
| Recurso | Mínimo | Recomendado |
|---|---|---|
| CPU | x86_64, 4 cores, VT-x/AMD-V | 16+ cores, EPYC/Xeon |
| RAM | 8 GiB | 64–256 GiB |
| Disco root | 40 GiB SSD | 240 GiB NVMe |
| Disco datos | 100 GiB | 2–8 TiB NVMe |
| NIC | 1 GbE (XDP genérico) | 10/25 GbE (XDP nativo) |
| SO base | Ubuntu 22.04 / Debian 12 / bare metal | Bare metal (ISO Caimán) |

### NICs con soporte XDP nativo (mejor rendimiento)
```
Intel:  i40e (X710), ice (E810), ixgbe (X550)
Mellanox: mlx5_core (ConnectX-4/5/6)
Netronome: nfp
```
Las NICs `virtio_net` / `vmxnet3` funcionan en modo genérico con ~10% de penalización.

### Cluster mínimo recomendado (producción)
```
3 nodos × 32c / 256 GiB / 2× NVMe 2 TiB / 25 GbE
```
Esto da: etcd con quórum, VSAN FTT=1, DRS σ automático, sin SPOF.

---

## Opción 1 — ISO Caimán (bare metal, recomendado)

### Descargar
```bash
curl -fsSL https://releases.caiman.io/caiman-0.1.0.iso -o caiman.iso
# Verificar integridad
sha256sum caiman.iso
# esperado: <SHA256 impreso en releases>
```

### Grabar en USB
```bash
# ⚠ REEMPLAZA /dev/sdX CON TU USB — esto borra el dispositivo completo
dd if=caiman.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

### Boot desde USB
1. Configurar BIOS/UEFI: boot order → USB primero
2. Secure Boot: **desactivar** (módulo kernel caiman_net no está firmado aún)
3. Seleccionar modo en el menú iPXE:
   - `Control plane` → primer nodo
   - `Worker node` → nodos adicionales
   - `All-in-one` → dev/testing en un solo nodo

El instalador detecta el rol automáticamente y ejecuta `install.sh`.

---

## Opción 2 — Script sobre Ubuntu/Debian existente

```bash
# ─── Control plane (primer nodo) ──────────────────────────────
curl -fsSL https://install.caiman.io | sudo bash -s -- \
  --role cp \
  --uplink eth0

# Al terminar, el script imprime el comando de join para workers:
# caiman join 192.168.1.10 --token <TOKEN>

# ─── Worker nodes (repetir en cada nodo) ──────────────────────
curl -fsSL https://install.caiman.io | sudo bash -s -- \
  --role worker \
  --join 192.168.1.10 \
  --token <TOKEN_DEL_CP> \
  --uplink eth1

# ─── All-in-one (dev / laptop) ────────────────────────────────
curl -fsSL https://install.caiman.io | sudo bash -s -- \
  --role aio
```

### Opciones del script
```
--role         cp | worker | aio
--join         IP del control plane  (required para --role worker)
--token        Token de kubeadm join (required para --role worker)
--uplink       Interfaz de red para XDP (default: eth0)
--pod-cidr     CIDR para pods (default: 10.244.0.0/16)
--skip-confirm Saltar confirmaciones interactivas
--dry-run      Imprimir pasos sin ejecutar
```

---

## Opción 3 — PXE / red

Útil para datacenters donde se provisionan muchos nodos a la vez.

```bash
# 1. Levantar servidor PXE (en cualquier máquina de la red)
docker run -d \
  --name caiman-pxe \
  -p 69:69/udp -p 80:80 \
  -v /var/tftp:/tftp \
  networkboot/dhcpd

# 2. Copiar archivos PXE
mkdir -p /var/tftp/caiman
cp caiman-install/pxe/boot.ipxe /var/tftp/caiman/
cp caiman.iso /var/tftp/caiman/
# extraer vmlinuz e initrd del ISO:
mount -o loop caiman.iso /mnt/iso
cp /mnt/iso/casper/vmlinuz  /var/tftp/caiman/
cp /mnt/iso/casper/initrd   /var/tftp/caiman/

# 3. Configurar DHCP (option 66 + 67)
# En tu router/DHCP server:
#   next-server: <IP del servidor PXE>
#   filename: caiman/boot.ipxe

# 4. Bootar los nodos desde red
# El menú iPXE aparece automáticamente
```

---

## Opción 4 — Autoinstall desatendido

Para instalar N nodos sin ninguna interacción manual.

```bash
# Servir el autoinstall.yaml desde HTTP
python3 -m http.server 8080 --directory caiman-install/scripts/ &

# Arrancar el ISO con parámetros de autoinstall en el kernel:
# En GRUB, editar la línea de boot y agregar:
autoinstall ds=nocloud-net;s=http://<tu-servidor>:8080/

# Alternativamente, crear un ISO con autoinstall embebido:
./caiman-install/scripts/make-autoinstall-iso.sh \
  --iso caiman.iso \
  --config autoinstall.yaml \
  --output caiman-auto.iso
```

---

## Lo que instala el script

```
1. Preflight        Verificar CPU VT-x, RAM, disco, NIC, drivers XDP
2. Sistema          swap off · kernel modules · sysctl · hugepages
3. Containerd       runtime CRI para Kubernetes
4. Kubernetes       kubelet · kubeadm · kubectl  v1.29
5. Binarios         caiman-vmm · caiman-cni · caiman-mcp · caiman · caiman-livemig
6. Kernel module    caiman_net.ko (compilado o descargado precompilado)
7. XDP programs     xdp_microseg.o · xdp_vm_router.o → /usr/local/lib/caiman/
8. Servicios        caiman-init · caiman-api (systemd)
9. Control plane    kubeadm init · etcd · kube-apiserver (solo --role cp/aio)
   o Worker join    kubeadm join (solo --role worker)
10. Stack Caimán   caiman-mcp DaemonSet · caiman-drs · microseg CRDs · monitoring
11. Verificación   caiman cluster status · kubectl get nodes
```

Tiempo total: **8–15 minutos** por nodo (dependiendo de velocidad de disco y red).

---

## Post-instalación

```bash
# Estado del cluster
caiman cluster status

# Listar nodos
caiman node list

# Conectar al dashboard (desde cualquier máquina de la red)
# http://<CP_IP>:3000

# Grafana
# http://<CP_IP>:3001  (admin / caiman)

# Configurar CLI en tu laptop
caiman config set api-url http://<CP_IP>:8765
caiman ping

# Crear primera VM
caiman vm create \
  --name test-01 \
  --mem 512 \
  --cpus 2 \
  --kernel /var/lib/caiman/vmlinux
```

---

## Troubleshooting

```bash
# Ver log de instalación
tail -f /var/log/caiman-install.log

# Estado de servicios
systemctl status caiman-init caiman-api kubelet containerd

# Verificar módulo kernel
lsmod | grep caiman_net
dmesg | grep caiman

# Verificar XDP
ip link show dev eth0 | grep xdp
bpftool net show

# Estado de etcd
etcdctl --endpoints=localhost:2379 member list

# Estado del cluster K8s
kubectl get nodes -o wide
kubectl get pods -n caiman-system
kubectl get pods -n kube-system

# Logs de pods
kubectl logs -n caiman-system daemonset/caiman-mcp
kubectl logs -n caiman-system deploy/caiman-drs
```

---

## Upgrade

```bash
# Upgrade de binarios Caimán (sin downtime en los VMs)
curl -fsSL https://install.caiman.io | sudo bash -s -- \
  --role $(cat /etc/caiman/role) \
  --skip-confirm

# Upgrade de Kubernetes (con drain del nodo)
caiman node drain $(hostname)
# ... actualizar kubeadm/kubelet/kubectl ...
caiman node uncordon $(hostname)
```

---

## Desinstalación

```bash
# ⚠ Esto elimina todo — VMs, datos, configuración
sudo kubeadm reset -f
sudo systemctl stop caiman-api caiman-init
sudo rm -rf /opt/caiman /var/lib/caiman /var/run/caiman
sudo apt-get remove -y kubelet kubeadm kubectl
sudo rm /etc/systemd/system/caiman-*.service
sudo systemctl daemon-reload
```
