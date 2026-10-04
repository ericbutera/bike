package storage

import "testing"

func TestParseQuotaLimitsUsesDefaultsAndOverrides(t *testing.T) {
	cases := []struct {
		name   string
		values map[string]string
		want   [4]int
	}{
		{
			name: "existing defaults",
			want: [4]int{200, 2000, 100, 1000},
		},
		{
			name: "all buckets configured",
			values: map[string]string{
				quotaOverall15MinuteEnv: "11",
				quotaOverallDailyEnv:    "12",
				quotaRead15MinuteEnv:    "13",
				quotaReadDailyEnv:       "14",
			},
			want: [4]int{11, 12, 13, 14},
		},
		{
			name: "partial override preserves defaults",
			values: map[string]string{
				quotaOverall15MinuteEnv: " 15 ",
				quotaReadDailyEnv:       "17",
			},
			want: [4]int{15, 2000, 100, 17},
		},
	}

	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			limits, err := ParseQuotaLimits(func(key string) (string, bool) {
				value, exists := test.values[key]
				return value, exists
			})
			if err != nil {
				t.Fatal(err)
			}

			buckets := (Quota{Limits: limits}).buckets()
			got := [4]int{buckets[2].limit, buckets[3].limit, buckets[0].limit, buckets[1].limit}
			if got != test.want {
				t.Fatalf("quota bucket limits = %v, want %v", got, test.want)
			}
		})
	}
}

func TestParseQuotaLimitsRejectsInvalidOverrides(t *testing.T) {
	for _, value := range []string{"not-a-number", "0", "-1"} {
		t.Run(value, func(t *testing.T) {
			_, err := ParseQuotaLimits(func(key string) (string, bool) {
				return value, key == quotaRead15MinuteEnv
			})
			if err == nil {
				t.Fatalf("ParseQuotaLimits(%q) error = nil, want validation error", value)
			}
		})
	}
}
