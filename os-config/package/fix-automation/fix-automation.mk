################################################################################
#
# fix-automation
#
################################################################################

FIX_AUTOMATION_VERSION = local
FIX_AUTOMATION_SITE    = $(BR2_EXTERNAL_LINUX_RESCUE_PATH)/..
FIX_AUTOMATION_SITE_METHOD = local

# Remove nested buildroot/ after rsync to prevent infinite rsync nesting
define FIX_AUTOMATION_RM_BUILDROOT
	rm -rf $(@D)/buildroot
endef
FIX_AUTOMATION_POST_RSYNC_HOOKS += FIX_AUTOMATION_RM_BUILDROOT

define FIX_AUTOMATION_BUILD_CMDS
	# Binary pre-built by CI — nothing to do here
endef

define FIX_AUTOMATION_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 \
		$(BR2_EXTERNAL_LINUX_RESCUE_PATH)/../target/x86_64-unknown-linux-musl/release/fix-automation \
		$(TARGET_DIR)/usr/bin/fix-automation
	# Install our inittab early (before target-finalize) so that Buildroot's
	# GENERIC_SERIAL sed always has a file to process.
	$(INSTALL) -D -m 0644 \
		$(BR2_EXTERNAL_LINUX_RESCUE_PATH)/rootfs_overlay/etc/inittab \
		$(TARGET_DIR)/etc/inittab
endef

$(eval $(generic-package))