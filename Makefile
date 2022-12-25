HISTORY_DIR=/var/lib
INSTALL_DIR=/usr/local/bin

all:
	@cargo build --release

# The install rule does not build as cargo should run as non-root
install:
	@cp -v target/release/launcher "$(INSTALL_DIR)/launcher"
	@mkdir -p $(HISTORY_DIR)
	@touch "$(HISTORY_DIR)/history"
	@chmod 666 "$(HISTORY_DIR)/history"

uninstall:
	@rm -vf "$(INSTALL_DIR)/launcher"
	@rm -vf "$(HISTORY_DIR)/history"
