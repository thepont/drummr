SUMMARY = "drummr Audio Engine"
DESCRIPTION = "Real-time mathematical drum synthesis engine written in Rust"
LICENSE = "MIT"
LIC_FILES_CHKSUM = "file://${COMMON_LICENSE_DIR}/MIT;md5=0835ade698e0bcf8506ecda2f7b4f302"

inherit cargo

# Point to the source code (in this repo)
SRC_URI = "git://github.com/viberbot/drummr.git;protocol=https;branch=master"
SRCREV = "${AUTOREV}"

S = "${WORKDIR}/git"

# Dependencies
DEPENDS += "alsa-lib"

# Hardware tuning for audio
EXTRA_OECARGO = "--features pi-optimized"

do_install:append() {
    install -d ${D}${bindir}
    install -m 0755 ${B}/target/${CARGO_TARGET_SUBDIR}/drummr ${D}${bindir}/drummr
    
    install -d ${D}${sysconfdir}/drummr
    cp -r ${S}/presets ${D}${sysconfdir}/drummr/
}
