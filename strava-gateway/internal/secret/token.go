package secret

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/base64"
	"errors"
)

var ErrInvalidKey = errors.New("TOKEN_ENCRYPTION_KEY must be a base64-encoded 32-byte key")

type TokenCipher struct {
	aead cipher.AEAD
}

func NewTokenCipher(encodedKey string) (TokenCipher, error) {
	key, err := base64.StdEncoding.DecodeString(encodedKey)
	if err != nil || len(key) != 32 {
		return TokenCipher{}, ErrInvalidKey
	}
	block, err := aes.NewCipher(key)
	if err != nil {
		return TokenCipher{}, err
	}
	aead, err := cipher.NewGCM(block)
	if err != nil {
		return TokenCipher{}, err
	}
	return TokenCipher{aead: aead}, nil
}

// Encrypt binds a credential to its Strava athlete ID. Ciphertexts cannot be
// swapped between connections without authentication failing.
func (tokenCipher TokenCipher) Encrypt(athleteID []byte, plaintext []byte) ([]byte, error) {
	nonce := make([]byte, tokenCipher.aead.NonceSize())
	if _, err := rand.Read(nonce); err != nil {
		return nil, err
	}
	return tokenCipher.aead.Seal(nonce, nonce, plaintext, athleteID), nil
}

func (tokenCipher TokenCipher) Decrypt(athleteID []byte, sealed []byte) ([]byte, error) {
	nonceSize := tokenCipher.aead.NonceSize()
	if len(sealed) < nonceSize {
		return nil, errors.New("invalid token ciphertext")
	}
	return tokenCipher.aead.Open(nil, sealed[:nonceSize], sealed[nonceSize:], athleteID)
}
