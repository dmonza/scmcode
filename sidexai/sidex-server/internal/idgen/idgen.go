// Package idgen generates random identifiers for the scmCode agent server.
//
// scmCode: a clean-room version-4 UUID generator used in place of
// github.com/google/uuid, so the server carries one less direct dependency.
// IDs follow RFC 9562 (version 4, variant 10xx) and draw all 16 bytes from
// crypto/rand.
package idgen

import (
	"crypto/rand"
	"encoding/hex"
)

// New returns a random version-4 UUID in canonical 8-4-4-4-12 lowercase form,
// e.g. "f47ac10b-58cc-4372-a567-0e02b2c3d479".
//
// It panics if the system entropy source fails: an identifier derived from a
// weaker source (time, PID) would be predictable or collision-prone, and there
// is no safe fallback. On Go 1.24+ crypto/rand.Read never returns an error.
func New() string {
	var b [16]byte
	if _, err := rand.Read(b[:]); err != nil {
		panic("idgen: crypto/rand unavailable: " + err.Error())
	}
	b[6] = (b[6] & 0x0f) | 0x40 // version 4
	b[8] = (b[8] & 0x3f) | 0x80 // variant 10xx

	var out [36]byte
	hex.Encode(out[0:8], b[0:4])
	out[8] = '-'
	hex.Encode(out[9:13], b[4:6])
	out[13] = '-'
	hex.Encode(out[14:18], b[6:8])
	out[18] = '-'
	hex.Encode(out[19:23], b[8:10])
	out[23] = '-'
	hex.Encode(out[24:36], b[10:16])
	return string(out[:])
}
