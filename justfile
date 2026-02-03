# Build the application in release mode
build:
    cargo build --release

# Install the application and desktop entry system-wide
install: build
    @echo "Installing rsiv to /usr/local/bin..."
    sudo install -m 755 target/release/rsiv /usr/local/bin/rsiv
    @echo "Installing desktop entry to /usr/local/share/applications..."
    sudo install -m 644 rsiv.desktop /usr/local/share/applications/rsiv.desktop
    @echo "Updating desktop database..."
    -sudo update-desktop-database /usr/local/share/applications
    @echo "Installation complete."

# Uninstall the application
uninstall:
    sudo rm -f /usr/local/bin/rsiv
    sudo rm -f /usr/local/share/applications/rsiv.desktop
