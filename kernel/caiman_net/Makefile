# SPDX-License-Identifier: GPL-2.0-only
#
# Makefile for caiman_net_mod kernel module
# Build: make -C /lib/modules/$(uname -r)/build M=$(pwd) modules
# Clean: make -C /lib/modules/$(uname -r)/build M=$(pwd) clean

KDIR ?= /lib/modules/$(shell uname -r)/build
PWD  := $(shell pwd)

obj-m += caiman_net_mod.o
caiman_net_mod-objs := main.o netlink.o xdp_loader.o

ccflags-y := -I$(src)

all:
	$(MAKE) -C $(KDIR) M=$(PWD) modules

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean

install:
	$(MAKE) -C $(KDIR) M=$(PWD) modules_install
	depmod -a

# For cross-compilation (e.g. building against a different kernel tree)
cross:
	$(MAKE) -C $(KDIR) M=$(PWD) ARCH=$(ARCH) CROSS_COMPILE=$(CROSS_COMPILE) modules
