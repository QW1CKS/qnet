package main

import (
	"fmt"
	"os"
	"os/exec"

	"github.com/QW1CKS/qnet/linter/pkg/validator"
	"github.com/spf13/cobra"
)
	Use:   "sbom [path]",
	Short: "Generate SBOM for QNet implementation",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		path := args[0]
		fmt.Printf("Generating SBOM for: %s\n", path)

		// Use syft to generate SBOM
		sbomPath := "sbom.json"
		cmd := exec.Command("syft", path, "-o", "json", "--file", sbomPath)
		if err := cmd.Run(); err != nil {
			fmt.Printf("Error generating SBOM: %v\n", err)
			os.Exit(1)
		}

		fmt.Printf("SBOM generated at: %s\n", sbomPath)
	},
}b.com/QW1CKS/qnet/linter/pkg/validator"
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

		// TODO: Integrate syft for SBOM generation
		fmt.Println("SBOM generation not yet implemented")
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
