package main

import (
	"context"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// The VM can request public image resources only; no account secrets or arbitrary destinations.
func loadPublicImage(address string) bool {
	u, err := url.Parse(address)
	if err != nil || u.Scheme != "https" || u.User != nil || u.Port() != "" {
		return false
	}
	allowed := u.Hostname() == "aistudio.google.com" || u.Hostname() == "www.google.com" ||
		u.Hostname() == "www.gstatic.com" || u.Hostname() == "ssl.gstatic.com"
	if !allowed {
		return false
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, address, nil)
	if err != nil {
		return false
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:152.0) Gecko/20100101 Firefox/152.0")
	req.Header.Set("Referer", "https://aistudio.google.com/")
	client := &http.Client{CheckRedirect: func(_ *http.Request, _ []*http.Request) error { return http.ErrUseLastResponse }}
	resp, err := client.Do(req)
	if err != nil {
		return false
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(io.LimitReader(resp.Body, (4<<20)+1))
	return err == nil && resp.StatusCode == 200 && len(data) > 0 && len(data) <= 4<<20 &&
		strings.HasPrefix(resp.Header.Get("Content-Type"), "image/")
}
