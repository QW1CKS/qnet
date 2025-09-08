package main

import (
	"fmt"
	"os"

	"github.com/QW1CKS/qnet/linter/pkg/validator"
	"github.com/spf13/cobra"
)

var rootCmd = &cobra.Command{
	Use:   "qnet-lint",
	Short: "QNet Spec Linter - Validates compliance with QNet specifications",
	Long: `QNet Spec Linter validates QNet implementations against the specification.
It checks for compliance in L2 framing, TemplateID, KEY_UPDATE, and BN-Ticket headers.`,
}

var validateCmd = &cobra.Command{
	Use:   "validate [path]",
	Short: "Validate a QNet implementation",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		path := args[0]
		fmt.Printf("Validating QNet implementation at: %s\n", path)

		errors := validator.Validate(path)
		if len(errors) > 0 {
			fmt.Println("Validation failed:")
			for _, err := range errors {
				fmt.Printf("  - %v\n", err)
			}
			os.Exit(1)
		}

		fmt.Println("All validations passed!")
	},
}

var sbomCmd = &cobra.Command{
	Use:   "sbom [path]",
	Short: "Generate SBOM for QNet implementation",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		path := args[0]
		fmt.Printf("Generating SBOM for: %s\n", path)

		// Note: Syft integration removed for simplicity
		// In production, install syft and use: syft path -o json --file sbom.json
		fmt.Println("SBOM generation requires external syft tool")
		fmt.Println("Install syft: https://github.com/anchore/syft")
		fmt.Println("Then run: syft", path, "-o json --file sbom.json")
	},
}

func init() {
	rootCmd.AddCommand(validateCmd)
	rootCmd.AddCommand(sbomCmd)
}

func main() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		os.Exit(1)
	}
}
