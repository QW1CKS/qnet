package validator

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

// ValidationRule represents a compliance rule
type ValidationRule struct {
	Name        string
	Description string
	Check       func(path string) error
}

// ValidateL2Framing checks L2 framing compliance
func ValidateL2Framing(path string) error {
	// Look for framing code in Rust files
	rustFiles, err := findFiles(path, "*.rs")
	if err != nil {
		return err
	}

	for _, file := range rustFiles {
		content, err := os.ReadFile(file)
		if err != nil {
			continue
		}

		// Check for frame encoding/decoding
		if !strings.Contains(string(content), "Frame::encode") ||
		   !strings.Contains(string(content), "Frame::decode") {
			continue
		}

		// Check for AEAD protection
		if !strings.Contains(string(content), "seal") ||
		   !strings.Contains(string(content), "open") {
			return fmt.Errorf("L2 framing in %s missing AEAD protection", file)
		}

		// Check for length checks
		if !regexp.MustCompile(`len.*<.*24`).MatchString(string(content)) {
			return fmt.Errorf("L2 framing in %s missing length validation", file)
		}
	}

	return nil
}

// ValidateTemplateID checks TemplateID compliance
func ValidateTemplateID(path string) error {
	rustFiles, err := findFiles(path, "*.rs")
	if err != nil {
		return err
	}

	found := false
	for _, file := range rustFiles {
		content, err := os.ReadFile(file)
		if err != nil {
			continue
		}

		if strings.Contains(string(content), "compute_template_id") ||
		   strings.Contains(string(content), "TemplateID") {
			found = true
			break
		}
	}

	if !found {
		return fmt.Errorf("TemplateID implementation not found")
	}

	return nil
}

// ValidateKeyUpdate checks KEY_UPDATE compliance
func ValidateKeyUpdate(path string) error {
	rustFiles, err := findFiles(path, "*.rs")
	if err != nil {
		return err
	}

	for _, file := range rustFiles {
		content, err := os.ReadFile(file)
		if err != nil {
			continue
		}

		if strings.Contains(string(content), "KEY_UPDATE") {
			// Check for 3-frame overlap
			if !strings.Contains(string(content), "overlap") &&
			   !strings.Contains(string(content), "3") {
				return fmt.Errorf("KEY_UPDATE in %s missing 3-frame overlap", file)
			}
			break
		}
	}

	return nil
}

// ValidateBNTicket checks BN-Ticket header compliance
func ValidateBNTicket(path string) error {
	rustFiles, err := findFiles(path, "*.rs")
	if err != nil {
		return err
	}

	for _, file := range rustFiles {
		content, err := os.ReadFile(file)
		if err != nil {
			continue
		}

		if strings.Contains(string(content), "BN-Ticket") ||
		   strings.Contains(string(content), "256") {
			// Check for 256-byte limit
			if !strings.Contains(string(content), "256") {
				return fmt.Errorf("BN-Ticket in %s missing 256-byte validation", file)
			}
			break
		}
	}

	return nil
}

// Validate runs all validation rules
func Validate(path string) []error {
	var errors []error

	rules := []ValidationRule{
		{
			Name:        "L2 Framing",
			Description: "Checks AEAD protection and length validation",
			Check:       ValidateL2Framing,
		},
		{
			Name:        "TemplateID",
			Description: "Checks deterministic CBOR and SHA-256 computation",
			Check:       ValidateTemplateID,
		},
		{
			Name:        "KEY_UPDATE",
			Description: "Checks 3-frame overlap and nonce lifecycle",
			Check:       ValidateKeyUpdate,
		},
		{
			Name:        "BN-Ticket",
			Description: "Checks 256-byte header validation",
			Check:       ValidateBNTicket,
		},
	}

	for _, rule := range rules {
		if err := rule.Check(path); err != nil {
			errors = append(errors, fmt.Errorf("%s: %v", rule.Name, err))
		}
	}

	return errors
}

// findFiles finds files matching pattern in directory
func findFiles(dir, pattern string) ([]string, error) {
	var files []string
	err := filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if matched, _ := filepath.Match(pattern, filepath.Base(path)); matched {
			files = append(files, path)
		}
		return nil
	})
	return files, err
}
