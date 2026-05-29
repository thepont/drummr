SUMMARY = "The minimalistic OS image for drummr"
LICENSE = "MIT"

inherit core-image

# Add our custom engine
IMAGE_INSTALL += "drummr-engine"

# Add audio essentials
IMAGE_INSTALL += "alsa-utils alsa-tools"

# Add SSH for debugging (can be removed for production)
IMAGE_INSTALL += "openssh"

# Make it real-time capable
IMAGE_FEATURES += "ssh-server-openssh"
DISTRO_FEATURES:append = " pam alsa"

# Force static IP for the drum kit network
# (Add network config here)
