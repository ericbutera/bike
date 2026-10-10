package auth

import (
	"crypto/sha256"
	"crypto/subtle"
	"strings"
)

type Token string

func (t Token) Authorized(authorization string) bool {
	if t == "" {
		return true
	}
	supplied := sha256.Sum256([]byte(strings.TrimPrefix(authorization, "Bearer ")))
	expected := sha256.Sum256([]byte(t))
	return subtle.ConstantTimeCompare(supplied[:], expected[:]) == 1
}
