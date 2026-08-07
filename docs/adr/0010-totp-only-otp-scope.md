# Limit OTP support to TOTP for now

`anchor` accepts raw TOTP seeds and provisioning URIs, normalizes them into canonical `otpauth://` data, and generates local codes. We chose to leave HOTP counter-based flows out of the initial design because they add mutable per-entry state and recovery complexity without a clear fit for the solo-user workflow this product targets.
