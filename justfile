build:
    cargo build --release

install: build
    @echo "Installing rsiv to /usr/local/bin..."
    sudo install -m 755 target/release/rsiv /usr/local/bin/rsiv
    @echo "Installing desktop entry to /usr/local/share/applications..."
    sudo install -m 644 rsiv.desktop /usr/local/share/applications/rsiv.desktop
    @echo "Updating desktop database..."
    -sudo update-desktop-database /usr/local/share/applications
    @echo "Installation complete."

uninstall:
    sudo rm -f /usr/local/bin/rsiv
    sudo rm -f /usr/local/share/applications/rsiv.desktop

update: build
    @if [ -f /usr/local/bin/rsiv ] && [ -f /usr/local/share/applications/rsiv.desktop ] && \
       cmp -s target/release/rsiv /usr/local/bin/rsiv && \
       cmp -s rsiv.desktop /usr/local/share/applications/rsiv.desktop; then \
        echo "already up to date :)"; \
    else \
        just install; \
    fi
