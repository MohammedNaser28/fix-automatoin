include $(sort $(wildcard $(BR2_EXTERNAL_LINUX_RESCUE_PATH)/package/*/*.mk))

# ── Ensure inittab exists before Buildroot's GENERIC_SERIAL sed ──────────────
# The generic getty sed in target-finalize expects /etc/inittab to exist.
# If our rootfs overlay hasn't been applied yet (hook ordering issue with
# Buildroot 2025.02.13), create a placeholder with the # GENERIC_SERIAL
# marker so the sed succeeds.
define FIX_AUTOMATION_ENSURE_INITTAB
	@if [ ! -f $(TARGET_DIR)/etc/inittab ]; then \
		mkdir -p $(TARGET_DIR)/etc && \
		printf '::sysinit:/etc/init.d/rcS\n# GENERIC_SERIAL\n::ctrlaltdel:/sbin/reboot\n::shutdown:/etc/init.d/rcK\n::restart:/sbin/init\n' > $(TARGET_DIR)/etc/inittab; \
	fi
endef
TARGET_FINALIZE_HOOKS += FIX_AUTOMATION_ENSURE_INITTAB
