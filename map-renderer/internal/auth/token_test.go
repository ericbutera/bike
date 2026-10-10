package auth

import (
	"github.com/stretchr/testify/require"
	"testing"
)

func TestTokenAuthorization(t *testing.T) {
	require.True(t, Token("").Authorized(""))
	for _, value := range []string{"Bearer secret", "secret"} {
		require.True(t, Token("secret").Authorized(value))
	}
	for _, value := range []string{"", "Bearer other", "bearer secret", "Bearer secret "} {
		require.False(t, Token("secret").Authorized(value))
	}
}
