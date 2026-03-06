package core

import (
	"os"
	"path/filepath"
	"testing"
)

func TestWithinBoundary(t *testing.T) {
	tmpDir := t.TempDir()
	fence := NewFence(tmpDir)
	fence.SetInteractive(false)

	// Path inside boundary should return true.
	inside := filepath.Join(tmpDir, "subdir", "file.txt")
	if !fence.IsWithinBoundary(inside) {
		t.Errorf("expected %q to be within boundary %q", inside, tmpDir)
	}

	// Boundary itself should return true.
	if !fence.IsWithinBoundary(tmpDir) {
		t.Errorf("expected boundary %q itself to be within boundary", tmpDir)
	}

	// Create a separate temp dir that is definitely outside the boundary.
	outsideDir := t.TempDir()
	outside := filepath.Join(outsideDir, "some_file.txt")
	if fence.IsWithinBoundary(outside) {
		t.Errorf("expected %q to be outside boundary %q", outside, tmpDir)
	}
}

func TestPrefixPathBypassPrevented(t *testing.T) {
	tmpDir := t.TempDir()

	// Create a boundary directory "ws".
	boundary := filepath.Join(tmpDir, "ws")
	os.MkdirAll(boundary, 0755)

	fence := NewFence(boundary)
	fence.SetInteractive(false)

	// "ws_evil" shares a prefix with "ws" but is outside the boundary.
	outsideWithSamePrefix := filepath.Join(tmpDir, "ws_evil", "file.txt")
	if fence.IsWithinBoundary(outsideWithSamePrefix) {
		t.Errorf("expected %q to be OUTSIDE boundary %q (prefix bypass)", outsideWithSamePrefix, boundary)
	}

	// Inside should still work.
	inside := filepath.Join(boundary, "repo", "file.txt")
	if !fence.IsWithinBoundary(inside) {
		t.Errorf("expected %q to be within boundary %q", inside, boundary)
	}
}

func TestCreateDirAllWithinBoundary(t *testing.T) {
	tmpDir := t.TempDir()
	fence := NewFence(tmpDir)
	fence.SetInteractive(false)

	newDir := filepath.Join(tmpDir, "test", "nested", "dir")
	err := fence.CreateDirAll(newDir)
	if err != nil {
		t.Fatalf("CreateDirAll() error: %v", err)
	}

	info, err := os.Stat(newDir)
	if err != nil {
		t.Fatalf("directory was not created: %v", err)
	}
	if !info.IsDir() {
		t.Error("expected a directory")
	}
}

func TestWriteOutsideBoundaryFails(t *testing.T) {
	tmpDir := t.TempDir()
	fence := NewFence(tmpDir)
	fence.SetInteractive(false)

	// Create a separate temp dir to use as an "outside" target.
	outsideDir := t.TempDir()
	outside := filepath.Join(outsideDir, "wtp_test_outside.txt")

	err := fence.Write(outside, []byte("test"))
	if err == nil {
		t.Error("expected error when writing outside boundary, got nil")
		// Clean up if somehow written.
		os.Remove(outside)
	}
}

func TestWriteWithinBoundary(t *testing.T) {
	tmpDir := t.TempDir()
	fence := NewFence(tmpDir)
	fence.SetInteractive(false)

	target := filepath.Join(tmpDir, "test.txt")
	err := fence.Write(target, []byte("hello"))
	if err != nil {
		t.Fatalf("Write() error: %v", err)
	}

	data, err := os.ReadFile(target)
	if err != nil {
		t.Fatalf("failed to read written file: %v", err)
	}
	if string(data) != "hello" {
		t.Errorf("file content = %q, want %q", string(data), "hello")
	}
}

func TestRemoveDirAllOutsideBoundaryFails(t *testing.T) {
	tmpDir := t.TempDir()
	fence := NewFence(tmpDir)
	fence.SetInteractive(false)

	outsideDir := t.TempDir()
	target := filepath.Join(outsideDir, "to_remove")
	os.MkdirAll(target, 0755)

	err := fence.RemoveDirAll(target)
	if err == nil {
		t.Error("expected error when removing outside boundary, got nil")
	}

	// Verify the directory still exists.
	if _, statErr := os.Stat(target); statErr != nil {
		t.Error("directory should still exist after failed remove")
	}
}

func TestFenceFromConfig(t *testing.T) {
	cfg := &GlobalConfig{
		WorkspaceRoot: "/tmp/test-workspaces",
	}
	fence := NewFenceFromConfig(cfg)
	if fence.Boundary() != "/tmp/test-workspaces" {
		t.Errorf("Boundary() = %q, want %q", fence.Boundary(), "/tmp/test-workspaces")
	}
}
