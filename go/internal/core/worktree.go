package core

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/google/uuid"
)

// RepoRefKind distinguishes between hosted and absolute repository references.
type RepoRefKind int

const (
	RepoRefHosted  RepoRefKind = iota // Repository relative to a host alias
	RepoRefAbsolute                    // Repository at an absolute path
)

// RepoRef references a git repository, either relative to a host alias or as
// an absolute path. Its TOML representation is compatible with Rust serde's
// externally-tagged enum format:
//
//	[repo.hosted]
//	host = "gh"
//	path = "owner/repo"
//
//	[repo.absolute]
//	path = "/abs/path"
type RepoRef struct {
	Kind RepoRefKind
	Host string // Only for Hosted
	Path string // Relative path (Hosted) or absolute path (Absolute)
}

// NewHostedRepoRef creates a RepoRef that is relative to a host alias.
func NewHostedRepoRef(host, path string) RepoRef {
	return RepoRef{Kind: RepoRefHosted, Host: host, Path: path}
}

// NewAbsoluteRepoRef creates a RepoRef with an absolute filesystem path.
func NewAbsoluteRepoRef(path string) RepoRef {
	return RepoRef{Kind: RepoRefAbsolute, Path: path}
}

// ToAbsolutePath resolves the RepoRef to an absolute path using host mappings.
func (r RepoRef) ToAbsolutePath(hosts map[string]string) string {
	switch r.Kind {
	case RepoRefHosted:
		if root, ok := hosts[r.Host]; ok {
			return filepath.Join(root, r.Path)
		}
		return r.Path
	default:
		return r.Path
	}
}

// Display returns a human-readable representation of the repo reference.
func (r RepoRef) Display() string {
	switch r.Kind {
	case RepoRefHosted:
		return fmt.Sprintf("%s:%s", r.Host, r.Path)
	default:
		return r.Path
	}
}

// Slug returns the last path component (repository name).
func (r RepoRef) Slug() string {
	name := filepath.Base(r.Path)
	if name == "." || name == string(filepath.Separator) || name == "" {
		return "unknown"
	}
	return name
}

// Equal reports whether two RepoRefs are identical.
func (r RepoRef) Equal(other RepoRef) bool {
	return r.Kind == other.Kind && r.Host == other.Host && r.Path == other.Path
}

// repoRefTOML is the intermediate TOML representation for RepoRef.
// Serde's externally-tagged enum serializes as { hosted: {host, path} } or
// { absolute: {path} }.
type repoRefTOML struct {
	Hosted  *repoRefHostedFields  `toml:"hosted,omitempty"`
	Absolute *repoRefAbsoluteFields `toml:"absolute,omitempty"`
}

type repoRefHostedFields struct {
	Host string `toml:"host"`
	Path string `toml:"path"`
}

type repoRefAbsoluteFields struct {
	Path string `toml:"path"`
}

// repoRefFromTOML converts the TOML intermediate representation into a RepoRef.
func repoRefFromTOML(raw repoRefTOML) (RepoRef, error) {
	switch {
	case raw.Hosted != nil:
		return NewHostedRepoRef(raw.Hosted.Host, raw.Hosted.Path), nil
	case raw.Absolute != nil:
		return NewAbsoluteRepoRef(raw.Absolute.Path), nil
	default:
		return RepoRef{}, fmt.Errorf("invalid repo ref: neither hosted nor absolute")
	}
}

// repoRefToTOML converts a RepoRef into the TOML intermediate representation.
func repoRefToTOML(r RepoRef) repoRefTOML {
	switch r.Kind {
	case RepoRefHosted:
		return repoRefTOML{Hosted: &repoRefHostedFields{Host: r.Host, Path: r.Path}}
	case RepoRefAbsolute:
		return repoRefTOML{Absolute: &repoRefAbsoluteFields{Path: r.Path}}
	default:
		return repoRefTOML{}
	}
}

// WorktreeEntry represents a single worktree in a workspace.
type WorktreeEntry struct {
	ID           string    // UUID
	Repo         RepoRef   // Reference to the original repository
	Branch       string    // Branch name
	WorktreePath string    // Path to the worktree directory (relative to workspace root)
	Base         string    // Base reference used when creating (optional, empty if none)
	HeadCommit   string    // HEAD commit hash at creation time (optional, empty if none)
	CreatedAt    time.Time // Creation timestamp
}

// NewWorktreeEntry creates a new WorktreeEntry with a generated UUID and
// current timestamp.
func NewWorktreeEntry(repo RepoRef, branch, worktreePath, base, headCommit string) WorktreeEntry {
	return WorktreeEntry{
		ID:           uuid.New().String(),
		Repo:         repo,
		Branch:       branch,
		WorktreePath: worktreePath,
		Base:         base,
		HeadCommit:   headCommit,
		CreatedAt:    time.Now(),
	}
}

// worktreeEntryTOML is the TOML-serializable form of WorktreeEntry, matching
// the Rust serde output exactly.
type worktreeEntryTOML struct {
	ID           string      `toml:"id"`
	Repo         repoRefTOML `toml:"repo"`
	Branch       string      `toml:"branch"`
	WorktreePath string      `toml:"worktree_path"`
	Base         *string     `toml:"base,omitempty"`
	HeadCommit   *string     `toml:"head_commit,omitempty"`
	CreatedAt    time.Time   `toml:"created_at"`
}

func worktreeEntryToTOML(e WorktreeEntry) worktreeEntryTOML {
	t := worktreeEntryTOML{
		ID:           e.ID,
		Repo:         repoRefToTOML(e.Repo),
		Branch:       e.Branch,
		WorktreePath: e.WorktreePath,
		CreatedAt:    e.CreatedAt,
	}
	if e.Base != "" {
		t.Base = &e.Base
	}
	if e.HeadCommit != "" {
		t.HeadCommit = &e.HeadCommit
	}
	return t
}

func worktreeEntryFromTOML(t worktreeEntryTOML) (WorktreeEntry, error) {
	repo, err := repoRefFromTOML(t.Repo)
	if err != nil {
		return WorktreeEntry{}, fmt.Errorf("parsing repo ref: %w", err)
	}
	e := WorktreeEntry{
		ID:           t.ID,
		Repo:         repo,
		Branch:       t.Branch,
		WorktreePath: t.WorktreePath,
		CreatedAt:    t.CreatedAt,
	}
	if t.Base != nil {
		e.Base = *t.Base
	}
	if t.HeadCommit != nil {
		e.HeadCommit = *t.HeadCommit
	}
	return e, nil
}

// WorktreeToml represents the .wtp/worktree.toml file structure.
type WorktreeToml struct {
	Version   string
	Worktrees []WorktreeEntry
}

// NewWorktreeToml creates an empty WorktreeToml with version "1".
func NewWorktreeToml() WorktreeToml {
	return WorktreeToml{
		Version:   "1",
		Worktrees: nil,
	}
}

// worktreeTomlTOML is the TOML-serializable form.
type worktreeTomlTOML struct {
	Version   string              `toml:"version"`
	Worktrees []worktreeEntryTOML `toml:"worktrees"`
}

func worktreeTomlToTOML(w WorktreeToml) worktreeTomlTOML {
	entries := make([]worktreeEntryTOML, len(w.Worktrees))
	for i, e := range w.Worktrees {
		entries[i] = worktreeEntryToTOML(e)
	}
	return worktreeTomlTOML{
		Version:   w.Version,
		Worktrees: entries,
	}
}

func worktreeTomlFromTOML(t worktreeTomlTOML) (WorktreeToml, error) {
	entries := make([]WorktreeEntry, 0, len(t.Worktrees))
	for _, raw := range t.Worktrees {
		e, err := worktreeEntryFromTOML(raw)
		if err != nil {
			return WorktreeToml{}, err
		}
		entries = append(entries, e)
	}
	return WorktreeToml{
		Version:   t.Version,
		Worktrees: entries,
	}, nil
}

// AddWorktree appends a worktree entry.
func (w *WorktreeToml) AddWorktree(entry WorktreeEntry) {
	w.Worktrees = append(w.Worktrees, entry)
}

// FindByRepo returns the first worktree matching the given RepoRef, or nil.
func (w *WorktreeToml) FindByRepo(repo RepoRef) *WorktreeEntry {
	for i := range w.Worktrees {
		if w.Worktrees[i].Repo.Equal(repo) {
			return &w.Worktrees[i]
		}
	}
	return nil
}

// FindBySlug returns the first worktree whose slug or display matches the
// given string, or nil.
func (w *WorktreeToml) FindBySlug(slug string) *WorktreeEntry {
	for i := range w.Worktrees {
		if w.Worktrees[i].Repo.Slug() == slug || w.Worktrees[i].Repo.Display() == slug {
			return &w.Worktrees[i]
		}
	}
	return nil
}

// RemoveBySlug removes the first worktree whose slug or display matches.
// Returns true if an entry was removed.
func (w *WorktreeToml) RemoveBySlug(slug string) bool {
	before := len(w.Worktrees)
	filtered := w.Worktrees[:0]
	for _, e := range w.Worktrees {
		if e.Repo.Slug() != slug && e.Repo.Display() != slug {
			filtered = append(filtered, e)
		}
	}
	w.Worktrees = filtered
	return len(w.Worktrees) < before
}

// LoadWorktreeToml reads and parses a worktree.toml file. If the file does
// not exist, it returns a new empty WorktreeToml.
func LoadWorktreeToml(path string) (WorktreeToml, error) {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return NewWorktreeToml(), nil
	}

	var raw worktreeTomlTOML
	if _, err := toml.DecodeFile(path, &raw); err != nil {
		return WorktreeToml{}, fmt.Errorf("decoding worktree.toml: %w", err)
	}

	return worktreeTomlFromTOML(raw)
}

// Save writes the WorktreeToml to disk. The optional writeFn allows the
// caller to inject a fence-aware writer; if nil, os.WriteFile is used.
func (w *WorktreeToml) Save(path string, writeFn func(path string, data []byte) error) error {
	raw := worktreeTomlToTOML(*w)

	buf, err := tomlMarshal(raw)
	if err != nil {
		return fmt.Errorf("encoding worktree.toml: %w", err)
	}

	if writeFn != nil {
		return writeFn(path, buf)
	}
	return os.WriteFile(path, buf, 0644)
}

// tomlMarshal encodes a value to TOML bytes using BurntSushi/toml.
func tomlMarshal(v interface{}) ([]byte, error) {
	var buf bytes.Buffer
	enc := toml.NewEncoder(&buf)
	if err := enc.Encode(v); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// WorktreeManager manages worktree entries for a single workspace.
type WorktreeManager struct {
	config     WorktreeToml
	configPath string
	writeFn    func(path string, data []byte) error // optional fence-aware writer
}

// LoadWorktreeManager loads the worktree metadata for the given workspace root.
func LoadWorktreeManager(workspaceRoot string, writeFn func(string, []byte) error) (*WorktreeManager, error) {
	configPath := filepath.Join(workspaceRoot, ".wtp", "worktree.toml")
	config, err := LoadWorktreeToml(configPath)
	if err != nil {
		return nil, err
	}
	return &WorktreeManager{
		config:     config,
		configPath: configPath,
		writeFn:    writeFn,
	}, nil
}

// Save persists the worktree metadata to disk.
func (m *WorktreeManager) Save() error {
	return m.config.Save(m.configPath, m.writeFn)
}

// Config returns a pointer to the underlying WorktreeToml.
func (m *WorktreeManager) Config() *WorktreeToml {
	return &m.config
}

// GenerateWorktreePath returns the relative worktree path for a repo slug.
func (m *WorktreeManager) GenerateWorktreePath(repoSlug string) string {
	return repoSlug
}

// ListWorktrees returns all worktree entries.
func (m *WorktreeManager) ListWorktrees() []WorktreeEntry {
	return m.config.Worktrees
}

// AddWorktree adds a worktree entry and saves.
func (m *WorktreeManager) AddWorktree(entry WorktreeEntry) error {
	m.config.AddWorktree(entry)
	return m.Save()
}

// RemoveWorktree removes a worktree by slug and saves if changed.
// Returns true if an entry was removed.
func (m *WorktreeManager) RemoveWorktree(slug string) (bool, error) {
	removed := m.config.RemoveBySlug(slug)
	if removed {
		if err := m.Save(); err != nil {
			return false, err
		}
	}
	return removed, nil
}
