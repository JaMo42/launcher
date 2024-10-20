INSTALL_DIR=/usr/local/bin

all:
	@cargo build --release

# The install rule does not build as cargo should run as non-root
install:
	@cp -v target/release/launcher "$(INSTALL_DIR)/launcher"

uninstall:
	@rm -vf "$(INSTALL_DIR)/launcher"
