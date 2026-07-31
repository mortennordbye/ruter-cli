# Security Policy

## Supported Versions

Only the latest release is actively supported with security updates.

## Reporting a Vulnerability

Please do not report security vulnerabilities through public GitHub issues.
Use the "Report a vulnerability" button under this repository's **Security** tab
(private vulnerability reporting is enabled). You will receive an acknowledgement
within 48 hours.

## Scope notes

`ruter` is a read-only command line client. It sends coordinates to Entur's public
journey planner and geocoder, and optionally to ipinfo.io when falling back to IP
geolocation. It stores no credentials, opens no listening socket, and the only file it
writes is its own config at `~/.config/ruter/config.toml`.

The macOS build embeds an `Info.plist` and is installed into an app bundle so it can hold
its own Location Services grant. That grant is the only system permission it requests.
