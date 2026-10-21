package auth

import (
	"mime"
	"net"
	"net/http"
	"strings"
)

// LocalGuard returns middleware that rejects, before any handler runs, the
// requests a web page could get a browser to send to this loopback server:
//
//   - A Host header that is not loopback, when hostMustBeLoopback is set.
//     This stops DNS rebinding: a page on a domain that re-resolves to
//     127.0.0.1 would otherwise be same-origin with the server and could
//     read sessions, transcripts and memory.
//   - An Origin header that is present but not in allowedOrigins. Browsers
//     send simple cross-origin requests (e.g. a POST with a text/plain body)
//     without a CORS preflight, so the CORS headers alone do not stop the
//     request from reaching a handler.
//   - A POST, PUT or PATCH whose Content-Type is not application/json. Every
//     handler decodes JSON, and requiring the JSON media type forces any
//     cross-origin request through a preflight that the CORS policy refuses.
//
// Requests without an Origin header (the desktop app's Rust side, the CLI,
// curl) are not affected by the Origin rule. The server still has no
// authentication of its own, so this does not protect against other
// processes or users on the same machine.
func LocalGuard(allowedOrigins map[string]bool, hostMustBeLoopback bool) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if hostMustBeLoopback && !IsLoopbackHost(r.Host) {
				http.Error(w, "forbidden: host is not loopback", http.StatusForbidden)
				return
			}
			if origin := r.Header.Get("Origin"); origin != "" && !allowedOrigins[origin] {
				http.Error(w, "forbidden: origin not allowed", http.StatusForbidden)
				return
			}
			switch r.Method {
			case http.MethodPost, http.MethodPut, http.MethodPatch:
				if !isJSONContentType(r.Header.Get("Content-Type")) {
					http.Error(w, "Content-Type must be application/json", http.StatusUnsupportedMediaType)
					return
				}
			}
			next.ServeHTTP(w, r)
		})
	}
}

func isJSONContentType(contentType string) bool {
	mediaType, _, err := mime.ParseMediaType(contentType)
	return err == nil && mediaType == "application/json"
}

// IsLoopbackHost reports whether host — as found in an HTTP request's Host
// header, with an optional ":port" — can only mean "this machine".
func IsLoopbackHost(host string) bool {
	h := host
	if hostOnly, _, err := net.SplitHostPort(host); err == nil {
		h = hostOnly
	}
	h = strings.Trim(h, "[]") // SplitHostPort only strips IPv6 brackets when a port is present
	if strings.EqualFold(h, "localhost") {
		return true
	}
	if ip := net.ParseIP(h); ip != nil {
		return ip.IsLoopback()
	}
	return false
}
