package worker

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestArtifactCanBeRestoredAfterCorruption(t *testing.T) {
	artifacts := Artifacts{Root: t.TempDir()}
	content := []byte(`{"activity":{"id":42},"streams":{}}`)
	stored, err := artifacts.Write(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(artifacts.Root, stored.RelativePath), []byte("corrupt"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := artifacts.ReadByHash(stored.SHA256); !errors.Is(err, ErrArtifactUnavailable) {
		t.Fatalf("corrupted artifact returned %v", err)
	}
	if _, err := artifacts.Write(content); err != nil {
		t.Fatal(err)
	}
	restored, err := artifacts.ReadByHash(stored.SHA256)
	if err != nil || !bytes.Equal(restored, content) {
		t.Fatalf("restored artifact did not match: %v", err)
	}
}
