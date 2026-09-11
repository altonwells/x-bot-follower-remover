forgive-me v0.1.4 adds automatic Chrome pairing and a guided setup flow.

- Press `b` in the terminal guide to open Chrome and copy the extension folder path.
- Load the extension once. Chrome still requires this installation step.
- The extension obtains the port and secret from a local native messaging helper. No manual secret entry is needed.
- The extension shows the terminal connection and X account separately. The terminal still requires account confirmation before work.
- Manual connection settings remain available for repair and survive reconnects.
- Cancelled pairing attempts cannot replace a newer connection choice.
- The installer preserves the extension path and stored data. Uninstall removes this installation's native host registration.

After updating, quit and reopen `forgive-me`. Reload the extension in `chrome://extensions` and accept the native messaging permission if Chrome asks. Open the extension and select **Connect automatically**.

Tests cover the native message exchange with the compiled executable, authenticated local connection, pairing races, setup screens, terminal behavior, and installation. Live Chrome installation and live X compatibility remain unverified. No real followers were removed.
