#!/bin/bash
# sync-guest-modules.sh -- copia módulos del guest desde Hetzner al repo
set -e

HETZNER="root@65.109.83.244"
KVER="6.6.69-0-virt"
DEST="/root/caiman-os/vmm/guest/initrd"

echo "=== Sincronizando módulos del guest desde Hetzner ==="

# Crear estructura
mkdir -p $DEST/lib/modules/$KVER/kernel/drivers/virtio
mkdir -p $DEST/lib/modules/$KVER/kernel/drivers/block
mkdir -p $DEST/lib/modules/$KVER/kernel/fs/ext4
mkdir -p $DEST/lib/modules/$KVER/kernel/fs/jbd2
mkdir -p $DEST/lib/modules/$KVER/kernel/crypto
mkdir -p $DEST/lib/modules/$KVER/kernel/lib
mkdir -p $DEST/bin

# Copiar init
scp $HETZNER:/tmp/caiman-initrd/init $DEST/
chmod +x $DEST/init

# Copiar módulos
# virtio_mmio is built into the kernel -- no module needed

scp $HETZNER:/tmp/caiman-initrd/lib/modules/$KVER/kernel/drivers/block/virtio_blk.ko \
    $DEST/lib/modules/$KVER/kernel/drivers/block/

scp $HETZNER:/tmp/caiman-initrd/lib/modules/$KVER/kernel/lib/crc16.ko \
    $DEST/lib/modules/$KVER/kernel/lib/

scp $HETZNER:/tmp/caiman-initrd/lib/modules/$KVER/kernel/crypto/crc32c_generic.ko \
    $DEST/lib/modules/$KVER/kernel/crypto/

scp $HETZNER:/tmp/caiman-initrd/lib/modules/$KVER/kernel/fs/mbcache.ko \
    $DEST/lib/modules/$KVER/kernel/fs/

scp $HETZNER:/tmp/caiman-initrd/lib/modules/$KVER/kernel/fs/jbd2/jbd2.ko \
    $DEST/lib/modules/$KVER/kernel/fs/jbd2/

scp $HETZNER:/tmp/caiman-initrd/lib/modules/$KVER/kernel/fs/ext4/ext4.ko \
    $DEST/lib/modules/$KVER/kernel/fs/ext4/

# Copiar busybox
scp $HETZNER:/tmp/caiman-initrd/bin/busybox \
    $DEST/bin/ 2>/dev/null || echo "WARN: busybox missing"

echo ""
echo "=== Módulos sincronizados ==="
find $DEST/lib -name "*.ko" | sort
echo ""
echo "Done."
