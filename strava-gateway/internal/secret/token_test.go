package secret

import (
	"bytes"
	"encoding/base64"
	"testing"
)

func TestTokenCipherBindsAthleteAndRandomizesCiphertext(t *testing.T) {
	cipher, err := NewTokenCipher(base64.StdEncoding.EncodeToString(bytes.Repeat([]byte{7}, 32)))
	if err != nil {
		t.Fatal(err)
	}
	first, err := cipher.Encrypt([]byte("123"), []byte("refresh-secret"))
	if err != nil {
		t.Fatal(err)
	}
	second, err := cipher.Encrypt([]byte("123"), []byte("refresh-secret"))
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Equal(first, second) {
		t.Fatal("ciphertext reused a nonce")
	}
	plain, err := cipher.Decrypt([]byte("123"), first)
	if err != nil || string(plain) != "refresh-secret" {
		t.Fatalf("round trip: %q %v", plain, err)
	}
	if _, err := cipher.Decrypt([]byte("456"), first); err == nil {
		t.Fatal("ciphertext was accepted for another athlete")
	}
}
