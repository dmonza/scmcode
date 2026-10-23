package idgen

import (
	"regexp"
	"testing"
)

var uuidV4Re = regexp.MustCompile(
	`^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`,
)

func TestNewFormat(t *testing.T) {
	for i := 0; i < 1000; i++ {
		id := New()
		if !uuidV4Re.MatchString(id) {
			t.Fatalf("New() = %q, not a canonical v4 UUID", id)
		}
	}
}

func TestNewUnique(t *testing.T) {
	const n = 10000
	seen := make(map[string]struct{}, n)
	for i := 0; i < n; i++ {
		id := New()
		if _, dup := seen[id]; dup {
			t.Fatalf("duplicate id after %d draws: %q", i, id)
		}
		seen[id] = struct{}{}
	}
}
