# QNet Spec Linter

A Go-based CLI tool that validates QNet implementations against the specification.

## Features

- **L2 Framing Validation**: Checks AEAD protection and length validation
- **TemplateID Validation**: Verifies deterministic CBOR and SHA-256 computation
- **KEY_UPDATE Validation**: Ensures 3-frame overlap and nonce lifecycle
- **BN-Ticket Validation**: Checks 256-byte header compliance
- **SBOM Generation**: Uses Syft to generate Software Bill of Materials

## Installation

```bash
cd linter
go mod download
go build -o qnet-lint ./cmd/qnet-lint
```

## Usage

### Validate Implementation
```bash
./qnet-lint validate /path/to/qnet/project
```

### Generate SBOM
```bash
./qnet-lint sbom /path/to/qnet/project
```

## Testing

Run the test script to verify the linter works:

```bash
cd linter
chmod +x test.sh
./test.sh
```

Expected output:
- Validation should pass for the QNet project
- SBOM file should be generated
- Clear error messages for any compliance issues

## GitHub Action

The linter is integrated into the CI pipeline via `.github/workflows/qnet-lint.yml`.

## Dependencies

- [Cobra](https://github.com/spf13/cobra) - CLI framework
- [Syft](https://github.com/anchore/syft) - SBOM generation
