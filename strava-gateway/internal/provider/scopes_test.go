package provider

import (
	"reflect"
	"testing"
)

func TestParseScopes(t *testing.T) {
	for _, input := range []string{"activity:read_all,read", "activity:read_all read", "activity:read_all, read"} {
		got := ParseScopes(input)
		want := []string{"activity:read_all", "read"}
		if !reflect.DeepEqual(got, want) || !HasScope([]string{input}, "activity:read_all") {
			t.Fatalf("scope %q parsed as %v", input, got)
		}
	}
}
