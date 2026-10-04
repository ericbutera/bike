package provider

import "strings"

// ParseScopes accepts the comma-separated OAuth response and the space-separated
// scope value returned by some token refresh responses. It also repairs scope
// values stored by older gateway versions as one array element.
func ParseScopes(value string) []string {
	return strings.FieldsFunc(value, func(r rune) bool {
		return r == ',' || r == ' ' || r == '\t' || r == '\n'
	})
}

func HasScope(scopes []string, wanted string) bool {
	for _, stored := range scopes {
		for _, scope := range ParseScopes(stored) {
			if scope == wanted {
				return true
			}
		}
	}
	return false
}
