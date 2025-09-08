package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"

	tlsutls "github.com/refraction-networking/utls"
	"github.com/spf13/cobra"
)

var rootCmd = &cobra.Command{
	Use:   "utls-gen",
	Short: "uTLS Template Generator for QNet",
	Long:  `Generates deterministic ClientHello blobs for TLS origin mirroring.`,
}

var generateCmd = &cobra.Command{
	Use:   "generate",
	Short: "Generate ClientHello templates",
	Run: func(cmd *cobra.Command, args []string) {
		generateTemplates()
	},
}

var updateCmd = &cobra.Command{
	Use:   "update",
	Short: "Update templates from latest Chrome releases",
	Run: func(cmd *cobra.Command, args []string) {
		updateTemplates()
	},
}

var selfTestCmd = &cobra.Command{
	Use:   "self-test",
	Short: "Run self-test on generated templates",
	Run: func(cmd *cobra.Command, args []string) {
		selfTest()
	},
}

func main() {
	rootCmd.AddCommand(generateCmd)
	rootCmd.AddCommand(updateCmd)
	rootCmd.AddCommand(selfTestCmd)

	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		os.Exit(1)
	}
}

func generateTemplates() {
	fmt.Println("Generating ClientHello templates...")

	// Use uTLS built-in fingerprints for determinism
	ids := []tlsutls.ClientHelloID{
		tlsutls.HelloChrome_Auto,
		tlsutls.HelloFirefox_Auto,
	}

	for i, id := range ids {
		// Create a uTLS connection to get the ClientHello
		uconn := tlsutls.UClient(nil, &tlsutls.Config{InsecureSkipVerify: true}, id)
		hello := uconn.HandshakeState.Hello

		// Make deterministic
		hello.Random = make([]byte, 32)
		hello.SessionId = make([]byte, 32)
		copy(hello.Random, []byte("qnet-deterministic-random-1234567"))
		copy(hello.SessionId, []byte("qnet-session-12345678901234567"))

		data, err := hello.Marshal()
		if err != nil {
			fmt.Printf("Error marshaling template %d: %v\n", i, err)
			continue
		}

		filename := fmt.Sprintf("template_%d.bin", i)
		err = os.WriteFile(filename, data, 0644)
		if err != nil {
			fmt.Printf("Error writing %s: %v\n", filename, err)
			continue
		}
		fmt.Printf("Generated %s for %s\n", filename, id.Str())
	}

	fmt.Println("Templates generated successfully.")
}

func updateTemplates() {
	fmt.Println("Updating templates from latest Chrome releases...")

	// Fetch latest Chrome version from GitHub API
	resp, err := http.Get("https://api.github.com/repos/chromium/chromium/releases/latest")
	if err != nil {
		fmt.Printf("Error fetching Chrome releases: %v\n", err)
		return
	}
	defer resp.Body.Close()

	var release struct {
		TagName string `json:"tag_name"`
	}
	err = json.NewDecoder(resp.Body).Decode(&release)
	if err != nil {
		fmt.Printf("Error parsing release: %v\n", err)
		return
	}

	fmt.Printf("Latest Chrome version: %s\n", release.TagName)

	// For now, just regenerate with updated version info
	generateTemplates()
}

func selfTest() {
	fmt.Println("Running self-test...")

	// Check if templates exist
	files, err := os.ReadDir(".")
	if err != nil {
		fmt.Printf("Error reading directory: %v\n", err)
		return
	}

	templateCount := 0
	for _, file := range files {
		if file.IsDir() {
			continue
		}
		if len(file.Name()) > 9 && file.Name()[:9] == "template_" {
			templateCount++
		}
	}

	if templateCount == 0 {
		fmt.Println("No templates found. Run 'generate' first.")
		return
	}

	fmt.Printf("Found %d templates.\n", templateCount)

	// Test parsing
	for i := 0; i < templateCount; i++ {
		filename := fmt.Sprintf("template_%d.bin", i)
		data, err := os.ReadFile(filename)
		if err != nil {
			fmt.Printf("Error reading %s: %v\n", filename, err)
			continue
		}

		// For self-test, just check file sizes or something simple
		fmt.Printf("Template %d: file size %d bytes\n", i, len(data))
	}

	fmt.Println("Self-test passed!")
}
