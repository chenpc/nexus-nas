# Yocto: create kerenl custom config

## generate .config
bitbake linux-yocto -c kernel_configme -f

## menuconfig
bitbake linux-yocto -c menuconfig

## make diff
bitbake linux-yocto -c diffconfig

## make file for recipes-kernel/linux/linux-yocto_%.bbappen

FILESEXTRAPATHS:prepend := "${THISDIR}/${PN}:"
SRC_URI += "file://fragmgnt.cfg"

# mount virtioFS
host# virtiofsd --socket-path=/var/run/vm001-vhost-fs.sock -o source=/var/lib/fs/vm001
host# qemu-system-x86_64 \
    -chardev socket,id=char0,path=/var/run/vm001-vhost-fs.sock \
    -device vhost-user-fs-pci,chardev=char0,tag=myfs \
    -object memory-backend-memfd,id=mem,size=4G,share=on \
    -numa node,memdev=mem \
    ...
guest# mount -t virtiofs myfs /mnt


# Real DEV
## client
/usr/lib/qemu/virtiofsd --socket-path=/tmp/vm001-vhost-fs.sock --shared-dir `pwd` --tag myfs

## qemu
runqemu nographic serialstdio slirp snapshot qemuparams="-chardev socket,id=char0,path=/tmp/vm001-vhost-fs.sock \
    -device vhost-user-fs-pci,chardev=char0,tag=myfs \
    -object memory-backend-memfd,id=mem,size=256M,share=on \
    -numa node,memdev=mem"
