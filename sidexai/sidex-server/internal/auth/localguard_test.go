package auth

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestLocalGuard(t *testing.T) {
	allowed := map[string]bool{"https://tauri.localhost": true}

	cases := []struct {
		name        string
		loopback    bool
		method      string
		host        string
		origin      string
		contentType string
		want        int
	}{
		{"app GET", true, "GET", "127.0.0.1:7433", "https://tauri.localhost", "", http.StatusOK},
		{"app POST json", true, "POST", "127.0.0.1:7433", "https://tauri.localhost", "application/json", http.StatusOK},
		{"json with charset", true, "POST", "127.0.0.1:7433", "", "application/json; charset=utf-8", http.StatusOK},
		{"no origin (Rust side, CLI)", true, "GET", "127.0.0.1:7433", "", "", http.StatusOK},
		{"localhost host", true, "GET", "localhost:7433", "", "", http.StatusOK},
		{"ipv6 loopback host", true, "GET", "[::1]:7433", "", "", http.StatusOK},
		{"preflight from app", true, "OPTIONS", "127.0.0.1:7433", "https://tauri.localhost", "", http.StatusOK},
		{"dns rebinding host", true, "GET", "attacker.example:7433", "http://attacker.example:7433", "", http.StatusForbidden},
		{"rebinding host without origin", true, "GET", "attacker.example:7433", "", "", http.StatusForbidden},
		{"foreign origin", true, "POST", "127.0.0.1:7433", "https://attacker.example", "application/json", http.StatusForbidden},
		{"null origin", true, "GET", "127.0.0.1:7433", "null", "", http.StatusForbidden},
		{"simple request text/plain", true, "POST", "127.0.0.1:7433", "", "text/plain", http.StatusUnsupportedMediaType},
		{"form post", true, "POST", "127.0.0.1:7433", "", "application/x-www-form-urlencoded", http.StatusUnsupportedMediaType},
		{"missing content type", true, "PUT", "127.0.0.1:7433", "", "", http.StatusUnsupportedMediaType},
		{"DELETE needs no body", true, "DELETE", "127.0.0.1:7433", "https://tauri.localhost", "", http.StatusOK},
		{"non-loopback bind skips host rule", false, "GET", "192.168.1.10:7433", "", "", http.StatusOK},
		{"non-loopback bind keeps origin rule", false, "GET", "192.168.1.10:7433", "https://attacker.example", "", http.StatusForbidden},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			h := LocalGuard(allowed, tc.loopback)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				w.WriteHeader(http.StatusOK)
			}))
			req := httptest.NewRequest(tc.method, "/v1/memory", nil)
			req.Host = tc.host
			if tc.origin != "" {
				req.Header.Set("Origin", tc.origin)
			}
			if tc.contentType != "" {
				req.Header.Set("Content-Type", tc.contentType)
			}
			rec := httptest.NewRecorder()
			h.ServeHTTP(rec, req)
			if rec.Code != tc.want {
				t.Fatalf("got %d, want %d", rec.Code, tc.want)
			}
		})
	}
}

func TestIsLoopbackHost(t *testing.T) {
	for host, want := range map[string]bool{
		"127.0.0.1":        true,
		"127.0.0.1:1234":   true,
		"localhost":        true,
		"LOCALHOST:80":     true,
		"[::1]:8080":       true,
		"::1":              true,
		"0.0.0.0:7433":     false,
		"192.168.0.2:7433": false,
		"example.com":      false,
		"":                 false,
	} {
		if got := IsLoopbackHost(host); got != want {
			t.Errorf("IsLoopbackHost(%q) = %v, want %v", host, got, want)
		}
	}
}
