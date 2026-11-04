# WinGet interface metadata

`Microsoft.Management.Deployment.winmd` is the official WinGet COM/WinRT interface metadata from Microsoft Desktop App Installer 1.29.380.0. It contains interface descriptions, not an installer or executable payload.

SHA-256: `b101a7a50b4698af369a57ec4cfbcd2df934333e2b4b8fe4ba27968f3faf7d69`.

Source: [microsoft/winget-cli](https://github.com/microsoft/winget-cli), MIT license; see `LICENSE`. The matching IDL is `src/Microsoft.Management.Deployment/PackageManager.idl`. `build.rs` generates private Rust projections with pinned `windows-bindgen` 0.61.1 and the existing Windows 0.61 ABI family. No installed WinGet SDK or network access is needed during generation. Runtime COM activation uses the published production CLSIDs, not WinRT in-process activation.
