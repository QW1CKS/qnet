# uTLS Template Generator

This tool generates deterministic ClientHello blobs for QNet's TLS origin mirroring feature. It uses the uTLS library to create fingerprints that mimic real browsers like Chrome and Firefox.

## Installation

Ensure Go 1.21+ is installed.

```bash
cd utls-gen
go build -o utls-gen.exe main.go
```

## Usage

### Generate Templates

Generate ClientHello templates for Chrome and Firefox:

```bash
./utls-gen.exe generate
```

This creates `template_0.bin` (Chrome) and `template_1.bin` (Firefox) with deterministic data for reproducibility.

### Update Templates

Fetch the latest Chrome version from GitHub and regenerate templates:

```bash
./utls-gen.exe update
```

### Self-Test

Verify that templates exist and are valid:

```bash
./utls-gen.exe self-test
```

## Features

- **Deterministic Output**: Uses fixed random and session ID data for consistent results across runs.
- **Browser Fingerprints**: Supports Chrome and Firefox ClientHello specs.
- **Auto-Update**: Fetches latest browser versions for template updates.
- **Self-Test**: Validates generated templates.

## Dependencies

- [uTLS](https://github.com/refraction-networking/utls): For TLS fingerprinting.
- [Cobra](https://github.com/spf13/cobra): For CLI commands.

## Integration with QNet

The generated templates can be used in QNet's TLS origin mirroring to blend with real browser traffic, enhancing stealth.
