# Use `otpauth://` as the canonical OTP form

`anchor` accepts raw OTP seeds and provisioning URIs, but stores and exports OTP data as an `otpauth://` URI. We chose this because it preserves interoperability with authenticator tools while keeping a single normalized representation for storage, sync, and recovery.
