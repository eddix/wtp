package core

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/BurntSushi/toml"
)

// --- RepoRef unit tests ---

func TestRepoRef_Slug(t *testing.T) {
	tests := []struct {
		name string
		ref  RepoRef
		want string
	}{
		{"hosted simple", NewHostedRepoRef("gh", "owner/repo"), "repo"},
		{"hosted deep", NewHostedRepoRef("gh", "org/team/repo"), "repo"},
		{"absolute unix", NewAbsoluteRepoRef("/home/user/projects/my-repo"), "my-repo"},
		{"absolute single", NewAbsoluteRepoRef("/repo"), "repo"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.ref.Slug()
			if got != tt.want {
				t.Errorf("Slug() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestRepoRef_Display(t *testing.T) {
	tests := []struct {
		name string
		ref  RepoRef
		want string
	}{
		{"hosted", NewHostedRepoRef("gh", "owner/repo"), "gh:owner/repo"},
		{"absolute", NewAbsoluteRepoRef("/home/user/repo"), "/home/user/repo"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.ref.Display()
			if got != tt.want {
				t.Errorf("Display() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestRepoRef_ToAbsolutePath(t *testing.T) {
	hosts := map[string]string{
		"gh": "/home/user/codes/github.com",
	}

	tests := []struct {
		name string
		ref  RepoRef
		want string
	}{
		{
			"hosted resolved",
			NewHostedRepoRef("gh", "owner/repo"),
			filepath.Join("/home/user/codes/github.com", "owner/repo"),
		},
		{
			"hosted unknown host fallback",
			NewHostedRepoRef("unknown", "owner/repo"),
			"owner/repo",
		},
		{
			"absolute passthrough",
			NewAbsoluteRepoRef("/some/path"),
			"/some/path",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.ref.ToAbsolutePath(hosts)
			if got != tt.want {
				t.Errorf("ToAbsolutePath() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestRepoRef_Equal(t *testing.T) {
	a := NewHostedRepoRef("gh", "owner/repo")
	b := NewHostedRepoRef("gh", "owner/repo")
	c := NewHostedRepoRef("gl", "owner/repo")
	d := NewAbsoluteRepoRef("/path")

	if !a.Equal(b) {
		t.Error("expected equal")
	}
	if a.Equal(c) {
		t.Error("expected not equal (different host)")
	}
	if a.Equal(d) {
		t.Error("expected not equal (different kind)")
	}
}

// --- RepoRef TOML serialization tests ---

// TestRepoRefTOML_HostedRoundTrip tests that a Hosted RepoRef survives
// marshal -> unmarshal and produces output compatible with Rust serde.
func TestRepoRefTOML_HostedRoundTrip(t *testing.T) {
	original := repoRefTOML{
		Hosted: &repoRefHostedFields{Host: "gh", Path: "owner/repo"},
	}

	data, err := tomlMarshal(original)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	tomlStr := string(data)

	// Verify it contains the expected keys under [hosted]
	if !strings.Contains(tomlStr, "[hosted]") {
		t.Errorf("expected [hosted] section in TOML output:\n%s", tomlStr)
	}
	if !strings.Contains(tomlStr, `host = "gh"`) {
		t.Errorf("expected host = \"gh\" in TOML output:\n%s", tomlStr)
	}
	if !strings.Contains(tomlStr, `path = "owner/repo"`) {
		t.Errorf("expected path = \"owner/repo\" in TOML output:\n%s", tomlStr)
	}

	var decoded repoRefTOML
	if _, err := toml.Decode(tomlStr, &decoded); err != nil {
		t.Fatalf("decode: %v", err)
	}

	ref, err := repoRefFromTOML(decoded)
	if err != nil {
		t.Fatalf("repoRefFromTOML: %v", err)
	}

	if ref.Kind != RepoRefHosted {
		t.Errorf("Kind = %d, want RepoRefHosted", ref.Kind)
	}
	if ref.Host != "gh" {
		t.Errorf("Host = %q, want \"gh\"", ref.Host)
	}
	if ref.Path != "owner/repo" {
		t.Errorf("Path = %q, want \"owner/repo\"", ref.Path)
	}
}

// TestRepoRefTOML_AbsoluteRoundTrip tests that an Absolute RepoRef survives
// marshal -> unmarshal and produces output compatible with Rust serde.
func TestRepoRefTOML_AbsoluteRoundTrip(t *testing.T) {
	original := repoRefTOML{
		Absolute: &repoRefAbsoluteFields{Path: "/home/user/repo"},
	}

	data, err := tomlMarshal(original)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	tomlStr := string(data)

	if !strings.Contains(tomlStr, "[absolute]") {
		t.Errorf("expected [absolute] section in TOML output:\n%s", tomlStr)
	}
	if !strings.Contains(tomlStr, `path = "/home/user/repo"`) {
		t.Errorf("expected path in TOML output:\n%s", tomlStr)
	}

	var decoded repoRefTOML
	if _, err := toml.Decode(tomlStr, &decoded); err != nil {
		t.Fatalf("decode: %v", err)
	}

	ref, err := repoRefFromTOML(decoded)
	if err != nil {
		t.Fatalf("repoRefFromTOML: %v", err)
	}

	if ref.Kind != RepoRefAbsolute {
		t.Errorf("Kind = %d, want RepoRefAbsolute", ref.Kind)
	}
	if ref.Path != "/home/user/repo" {
		t.Errorf("Path = %q, want \"/home/user/repo\"", ref.Path)
	}
}

// TestRepoRefTOML_InvalidEmpty tests decoding an empty repo ref.
func TestRepoRefTOML_InvalidEmpty(t *testing.T) {
	_, err := repoRefFromTOML(repoRefTOML{})
	if err == nil {
		t.Error("expected error for empty repoRefTOML")
	}
}

// --- WorktreeToml TOML round-trip test ---

// TestWorktreeToml_FullRoundTrip tests a complete WorktreeToml with multiple
// entries through marshal -> unmarshal, verifying Rust-compatible TOML.
func TestWorktreeToml_FullRoundTrip(t *testing.T) {
	ts := time.Date(2025, 3, 1, 12, 0, 0, 0, time.UTC)
	original := WorktreeToml{
		Version: "1",
		Worktrees: []WorktreeEntry{
			{
				ID:           "550e8400-e29b-41d4-a716-446655440000",
				Repo:         NewHostedRepoRef("gh", "acme/frontend"),
				Branch:       "feature-x",
				WorktreePath: "frontend",
				Base:         "main",
				HeadCommit:   "abc1234",
				CreatedAt:    ts,
			},
			{
				ID:           "550e8400-e29b-41d4-a716-446655440001",
				Repo:         NewAbsoluteRepoRef("/home/dev/backend"),
				Branch:       "feature-x",
				WorktreePath: "backend",
				Base:         "",
				HeadCommit:   "",
				CreatedAt:    ts,
			},
		},
	}

	// Marshal
	raw := worktreeTomlToTOML(original)
	data, err := tomlMarshal(raw)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	tomlStr := string(data)

	// Check TOML structure has Rust-compatible keys
	if !strings.Contains(tomlStr, `version = "1"`) {
		t.Errorf("missing version in output:\n%s", tomlStr)
	}
	if !strings.Contains(tomlStr, "[[worktrees]]") {
		t.Errorf("missing [[worktrees]] array in output:\n%s", tomlStr)
	}
	if !strings.Contains(tomlStr, `[worktrees.repo.hosted]`) {
		t.Errorf("missing [worktrees.repo.hosted] in output:\n%s", tomlStr)
	}
	if !strings.Contains(tomlStr, `[worktrees.repo.absolute]`) {
		t.Errorf("missing [worktrees.repo.absolute] in output:\n%s", tomlStr)
	}
	if !strings.Contains(tomlStr, `worktree_path = "frontend"`) {
		t.Errorf("missing worktree_path field in output:\n%s", tomlStr)
	}

	// Verify optional fields: base/head_commit should be absent for empty strings
	// (the second entry has empty base and head_commit)

	// Unmarshal
	var decoded worktreeTomlTOML
	if _, err := toml.Decode(tomlStr, &decoded); err != nil {
		t.Fatalf("decode: %v", err)
	}

	result, err := worktreeTomlFromTOML(decoded)
	if err != nil {
		t.Fatalf("worktreeTomlFromTOML: %v", err)
	}

	if result.Version != "1" {
		t.Errorf("Version = %q, want \"1\"", result.Version)
	}
	if len(result.Worktrees) != 2 {
		t.Fatalf("len(Worktrees) = %d, want 2", len(result.Worktrees))
	}

	// Check first entry (hosted)
	e0 := result.Worktrees[0]
	if e0.ID != "550e8400-e29b-41d4-a716-446655440000" {
		t.Errorf("Worktrees[0].ID = %q", e0.ID)
	}
	if e0.Repo.Kind != RepoRefHosted || e0.Repo.Host != "gh" || e0.Repo.Path != "acme/frontend" {
		t.Errorf("Worktrees[0].Repo = %+v", e0.Repo)
	}
	if e0.Branch != "feature-x" {
		t.Errorf("Worktrees[0].Branch = %q", e0.Branch)
	}
	if e0.WorktreePath != "frontend" {
		t.Errorf("Worktrees[0].WorktreePath = %q", e0.WorktreePath)
	}
	if e0.Base != "main" {
		t.Errorf("Worktrees[0].Base = %q, want \"main\"", e0.Base)
	}
	if e0.HeadCommit != "abc1234" {
		t.Errorf("Worktrees[0].HeadCommit = %q, want \"abc1234\"", e0.HeadCommit)
	}

	// Check second entry (absolute, no base/head_commit)
	e1 := result.Worktrees[1]
	if e1.Repo.Kind != RepoRefAbsolute || e1.Repo.Path != "/home/dev/backend" {
		t.Errorf("Worktrees[1].Repo = %+v", e1.Repo)
	}
	if e1.Base != "" {
		t.Errorf("Worktrees[1].Base = %q, want empty", e1.Base)
	}
	if e1.HeadCommit != "" {
		t.Errorf("Worktrees[1].HeadCommit = %q, want empty", e1.HeadCommit)
	}
}

// TestWorktreeToml_DecodeRustFormat tests that we can decode TOML produced by
// the Rust version of wtp.
func TestWorktreeToml_DecodeRustFormat(t *testing.T) {
	// This is the exact format that Rust's toml::to_string_pretty produces
	// for a WorktreeToml with serde's externally-tagged enum and snake_case.
	rustTOML := `version = "1"

[[worktrees]]
id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
branch = "feature-x"
worktree_path = "my-repo"
base = "main"
head_commit = "deadbeef1234567890"
created_at = 2025-06-15T10:30:00Z

[worktrees.repo.hosted]
host = "gh"
path = "acme/my-repo"

[[worktrees]]
id = "11111111-2222-3333-4444-555555555555"
branch = "hotfix-1"
worktree_path = "backend"
created_at = 2025-06-15T11:00:00Z

[worktrees.repo.absolute]
path = "/home/dev/backend"
`

	var raw worktreeTomlTOML
	if _, err := toml.Decode(rustTOML, &raw); err != nil {
		t.Fatalf("decode: %v", err)
	}

	result, err := worktreeTomlFromTOML(raw)
	if err != nil {
		t.Fatalf("worktreeTomlFromTOML: %v", err)
	}

	if result.Version != "1" {
		t.Errorf("Version = %q", result.Version)
	}
	if len(result.Worktrees) != 2 {
		t.Fatalf("len(Worktrees) = %d, want 2", len(result.Worktrees))
	}

	// First: hosted
	e0 := result.Worktrees[0]
	if e0.Repo.Kind != RepoRefHosted {
		t.Errorf("Worktrees[0].Repo.Kind = %d, want Hosted", e0.Repo.Kind)
	}
	if e0.Repo.Host != "gh" || e0.Repo.Path != "acme/my-repo" {
		t.Errorf("Worktrees[0].Repo = %+v", e0.Repo)
	}
	if e0.Base != "main" {
		t.Errorf("Worktrees[0].Base = %q", e0.Base)
	}
	if e0.HeadCommit != "deadbeef1234567890" {
		t.Errorf("Worktrees[0].HeadCommit = %q", e0.HeadCommit)
	}

	// Second: absolute, no base/head_commit
	e1 := result.Worktrees[1]
	if e1.Repo.Kind != RepoRefAbsolute {
		t.Errorf("Worktrees[1].Repo.Kind = %d, want Absolute", e1.Repo.Kind)
	}
	if e1.Repo.Path != "/home/dev/backend" {
		t.Errorf("Worktrees[1].Repo.Path = %q", e1.Repo.Path)
	}
	if e1.Base != "" {
		t.Errorf("Worktrees[1].Base = %q, want empty", e1.Base)
	}
	if e1.HeadCommit != "" {
		t.Errorf("Worktrees[1].HeadCommit = %q, want empty", e1.HeadCommit)
	}
}

// --- WorktreeToml CRUD tests ---

func TestWorktreeToml_AddFindRemove(t *testing.T) {
	wt := NewWorktreeToml()

	e1 := WorktreeEntry{
		ID:           "id-1",
		Repo:         NewHostedRepoRef("gh", "acme/frontend"),
		Branch:       "feat",
		WorktreePath: "frontend",
		CreatedAt:    time.Now(),
	}
	e2 := WorktreeEntry{
		ID:           "id-2",
		Repo:         NewAbsoluteRepoRef("/home/dev/backend"),
		Branch:       "feat",
		WorktreePath: "backend",
		CreatedAt:    time.Now(),
	}

	// Add
	wt.AddWorktree(e1)
	wt.AddWorktree(e2)
	if len(wt.Worktrees) != 2 {
		t.Fatalf("len = %d after adding 2", len(wt.Worktrees))
	}

	// FindByRepo
	found := wt.FindByRepo(NewHostedRepoRef("gh", "acme/frontend"))
	if found == nil {
		t.Fatal("FindByRepo returned nil for hosted ref")
	}
	if found.ID != "id-1" {
		t.Errorf("FindByRepo ID = %q, want id-1", found.ID)
	}

	// FindByRepo miss
	if wt.FindByRepo(NewHostedRepoRef("gl", "other/repo")) != nil {
		t.Error("FindByRepo should return nil for non-existent repo")
	}

	// FindBySlug
	found = wt.FindBySlug("frontend")
	if found == nil {
		t.Fatal("FindBySlug(frontend) returned nil")
	}
	if found.ID != "id-1" {
		t.Errorf("FindBySlug ID = %q, want id-1", found.ID)
	}

	// FindBySlug with display string
	found = wt.FindBySlug("gh:acme/frontend")
	if found == nil {
		t.Fatal("FindBySlug(gh:acme/frontend) returned nil")
	}

	// FindBySlug miss
	if wt.FindBySlug("nonexistent") != nil {
		t.Error("FindBySlug should return nil for non-existent slug")
	}

	// RemoveBySlug
	removed := wt.RemoveBySlug("frontend")
	if !removed {
		t.Error("RemoveBySlug returned false")
	}
	if len(wt.Worktrees) != 1 {
		t.Errorf("len = %d after remove, want 1", len(wt.Worktrees))
	}
	if wt.Worktrees[0].ID != "id-2" {
		t.Error("wrong entry remaining after remove")
	}

	// RemoveBySlug miss
	removed = wt.RemoveBySlug("nonexistent")
	if removed {
		t.Error("RemoveBySlug should return false for non-existent slug")
	}

	// RemoveBySlug with display string
	removed = wt.RemoveBySlug("/home/dev/backend")
	if !removed {
		t.Error("RemoveBySlug with display returned false")
	}
	if len(wt.Worktrees) != 0 {
		t.Errorf("len = %d after second remove, want 0", len(wt.Worktrees))
	}
}

// --- WorktreeManager file I/O tests ---

func TestWorktreeManager_LoadSaveRoundTrip(t *testing.T) {
	tmpDir := t.TempDir()

	// Create .wtp directory structure
	wtpDir := filepath.Join(tmpDir, ".wtp")
	if err := os.MkdirAll(wtpDir, 0755); err != nil {
		t.Fatal(err)
	}

	// Load from non-existent file -> empty
	mgr, err := LoadWorktreeManager(tmpDir, nil)
	if err != nil {
		t.Fatalf("LoadWorktreeManager: %v", err)
	}
	if mgr.Config().Version != "1" {
		t.Errorf("Version = %q, want \"1\"", mgr.Config().Version)
	}
	if len(mgr.ListWorktrees()) != 0 {
		t.Error("expected empty worktrees")
	}

	// Add entries and save
	entry := WorktreeEntry{
		ID:           "test-uuid-1",
		Repo:         NewHostedRepoRef("gh", "org/proj"),
		Branch:       "feature-a",
		WorktreePath: "proj",
		Base:         "main",
		HeadCommit:   "abc123",
		CreatedAt:    time.Date(2025, 6, 1, 0, 0, 0, 0, time.UTC),
	}
	if err := mgr.AddWorktree(entry); err != nil {
		t.Fatalf("AddWorktree: %v", err)
	}

	// Reload and verify
	mgr2, err := LoadWorktreeManager(tmpDir, nil)
	if err != nil {
		t.Fatalf("reload: %v", err)
	}
	wts := mgr2.ListWorktrees()
	if len(wts) != 1 {
		t.Fatalf("len = %d after reload, want 1", len(wts))
	}
	if wts[0].ID != "test-uuid-1" {
		t.Errorf("ID = %q", wts[0].ID)
	}
	if wts[0].Repo.Kind != RepoRefHosted || wts[0].Repo.Host != "gh" {
		t.Errorf("Repo = %+v", wts[0].Repo)
	}
	if wts[0].Base != "main" {
		t.Errorf("Base = %q", wts[0].Base)
	}

	// Remove and verify
	removed, err := mgr2.RemoveWorktree("proj")
	if err != nil {
		t.Fatalf("RemoveWorktree: %v", err)
	}
	if !removed {
		t.Error("RemoveWorktree returned false")
	}

	// Reload again
	mgr3, err := LoadWorktreeManager(tmpDir, nil)
	if err != nil {
		t.Fatalf("reload after remove: %v", err)
	}
	if len(mgr3.ListWorktrees()) != 0 {
		t.Error("expected 0 worktrees after remove + reload")
	}
}

func TestWorktreeManager_GenerateWorktreePath(t *testing.T) {
	mgr := &WorktreeManager{}
	got := mgr.GenerateWorktreePath("my-repo")
	if got != "my-repo" {
		t.Errorf("GenerateWorktreePath = %q, want \"my-repo\"", got)
	}
}

func TestWorktreeManager_SaveWithWriteFn(t *testing.T) {
	tmpDir := t.TempDir()
	wtpDir := filepath.Join(tmpDir, ".wtp")
	if err := os.MkdirAll(wtpDir, 0755); err != nil {
		t.Fatal(err)
	}

	var capturedPath string
	var capturedData []byte
	writeFn := func(path string, data []byte) error {
		capturedPath = path
		capturedData = data
		return os.WriteFile(path, data, 0644)
	}

	mgr, err := LoadWorktreeManager(tmpDir, writeFn)
	if err != nil {
		t.Fatal(err)
	}

	entry := WorktreeEntry{
		ID:           "uuid-fence",
		Repo:         NewAbsoluteRepoRef("/repos/test"),
		Branch:       "main",
		WorktreePath: "test",
		CreatedAt:    time.Now(),
	}
	if err := mgr.AddWorktree(entry); err != nil {
		t.Fatal(err)
	}

	expectedPath := filepath.Join(wtpDir, "worktree.toml")
	if capturedPath != expectedPath {
		t.Errorf("writeFn path = %q, want %q", capturedPath, expectedPath)
	}
	if len(capturedData) == 0 {
		t.Error("writeFn received empty data")
	}
}

func TestNewWorktreeEntry_GeneratesUUID(t *testing.T) {
	e := NewWorktreeEntry(
		NewHostedRepoRef("gh", "o/r"),
		"main",
		"r",
		"",
		"",
	)
	if e.ID == "" {
		t.Error("NewWorktreeEntry generated empty ID")
	}
	if len(e.ID) < 36 {
		t.Errorf("ID looks too short for UUID: %q", e.ID)
	}
	if e.CreatedAt.IsZero() {
		t.Error("CreatedAt is zero")
	}
}

func TestLoadWorktreeToml_NonExistentFile(t *testing.T) {
	wt, err := LoadWorktreeToml("/nonexistent/path/worktree.toml")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if wt.Version != "1" {
		t.Errorf("Version = %q, want \"1\"", wt.Version)
	}
	if len(wt.Worktrees) != 0 {
		t.Error("expected empty worktrees for non-existent file")
	}
}
