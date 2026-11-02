package main

import (
	"bufio"
	"bytes"
	"crypto/tls"
	"crypto/x509"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"net/http/httputil"
	"net/url"
	"strings"
	"sync"
	"time"
)

// proxy is an HTTP proxy that only accepts CONNECT. It intercepts the GitHub hosts and tunnels
// every other host to the upstream proxy.
type proxy struct {
	upstream *url.URL
	ca       *authority
	backend  *gitBackend
	forward  http.Handler
	log      *log.Logger
}

func newProxy(upstream *url.URL, ca *authority, backend *gitBackend, roots *x509.CertPool, logger *log.Logger) *proxy {
	transport := &http.Transport{
		Proxy:               http.ProxyURL(upstream),
		TLSClientConfig:     &tls.Config{RootCAs: roots},
		ForceAttemptHTTP2:   true,
		MaxIdleConnsPerHost: 8,
		IdleConnTimeout:     90 * time.Second,
	}
	forward := &httputil.ReverseProxy{
		Rewrite: func(r *httputil.ProxyRequest) {
			r.Out.URL.Scheme = "https"
			r.Out.URL.Host = r.In.Host
			r.Out.Host = r.In.Host
		},
		Transport: transport,
		ErrorLog:  logger,
	}
	return &proxy{upstream: upstream, ca: ca, backend: backend, forward: forward, log: logger}
}

func (p *proxy) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodConnect {
		http.Error(w, "ghshim only supports CONNECT", http.StatusMethodNotAllowed)
		return
	}
	host, port, err := net.SplitHostPort(r.Host)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	conn, rw, err := http.NewResponseController(w).Hijack()
	if err != nil {
		p.log.Printf("hijack: %v", err)
		return
	}
	client := withBuffered(conn, rw.Reader)
	if githubHosts[host] && port == "443" {
		p.intercept(client, host)
		return
	}
	p.tunnel(client, r.Host)
}

// tunnel relays the connection to target through the upstream proxy byte for byte.
func (p *proxy) tunnel(client net.Conn, target string) {
	defer client.Close()
	upstream, err := dialThroughUpstream(p.upstream, target)
	if err != nil {
		p.log.Printf("tunnel %s: %v", target, err)
		var status *upstreamStatusError
		if errors.As(err, &status) {
			fmt.Fprintf(client, "HTTP/1.1 %s\r\nContent-Length: 0\r\n\r\n", status.status)
		} else {
			io.WriteString(client, "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
		}
		return
	}
	defer upstream.Close()
	if _, err := io.WriteString(client, "HTTP/1.1 200 Connection Established\r\n\r\n"); err != nil {
		return
	}
	var wg sync.WaitGroup
	wg.Add(2)
	go func() { defer wg.Done(); copyAndCloseWrite(upstream, client) }()
	go func() { defer wg.Done(); copyAndCloseWrite(client, upstream) }()
	wg.Wait()
}

// intercept terminates TLS for a GitHub host and serves the HTTP requests sent over it.
func (p *proxy) intercept(client net.Conn, host string) {
	if _, err := io.WriteString(client, "HTTP/1.1 200 Connection Established\r\n\r\n"); err != nil {
		client.Close()
		return
	}
	tlsConn := tls.Server(client, &tls.Config{
		GetCertificate: func(hello *tls.ClientHelloInfo) (*tls.Certificate, error) {
			name := hello.ServerName
			if name == "" {
				name = host
			}
			if !githubHosts[name] {
				return nil, fmt.Errorf("no certificate for %q", name)
			}
			return p.ca.leaf(name)
		},
		// Serve over HTTP/1.1 only, which http.Server can do on a single connection.
		NextProtos: []string{"http/1.1"},
	})
	listener := newSingleConnListener(tlsConn)
	server := &http.Server{
		Handler:           http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { p.serveGitHub(w, r, host) }),
		ErrorLog:          p.log,
		ReadHeaderTimeout: time.Minute,
		ConnState:         listener.connState,
	}
	_ = server.Serve(listener)
}

func (p *proxy) serveGitHub(w http.ResponseWriter, r *http.Request, host string) {
	r.Host = host
	if r.Method == http.MethodGet {
		if req, ok := matchGitHubRequest(host, r.URL.Path); ok {
			err := p.serveFromGit(w, r, req)
			switch {
			case err == nil:
				return
			case errors.Is(err, errRefNotFound):
				writeGitHubError(w, http.StatusNotFound, err.Error())
				return
			case errors.Is(err, errUnreadable):
				p.log.Printf("%s %s: %v; forwarding", r.Method, r.URL.Path, err)
			default:
				p.log.Printf("%s %s: %v", r.Method, r.URL.Path, err)
				writeGitHubError(w, http.StatusBadGateway, err.Error())
				return
			}
		}
	}
	p.forward.ServeHTTP(w, r)
}

// serveFromGit answers req, or returns an error before writing anything to w.
func (p *proxy) serveFromGit(w http.ResponseWriter, r *http.Request, req githubRequest) error {
	ctx := r.Context()
	switch req.kind {
	case kindCommit:
		info, err := p.backend.commit(ctx, req.owner, req.repo, req.ref)
		if err != nil {
			return err
		}
		p.log.Printf("commit %s/%s %s -> %s", req.owner, req.repo, req.ref, info.sha)
		if strings.Contains(r.Header.Get("Accept"), "sha") {
			w.Header().Set("Content-Type", "application/vnd.github.sha")
			io.WriteString(w, info.sha)
			return nil
		}
		body, err := json.Marshal(map[string]any{
			"sha": info.sha,
			"commit": map[string]any{
				"tree":      map[string]string{"sha": info.tree},
				"committer": map[string]string{"date": info.date},
			},
		})
		if err != nil {
			return err
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		w.Write(body)
		return nil
	case kindTarball:
		dir, sha, err := p.backend.prepareArchive(ctx, req.owner, req.repo, req.ref)
		if err != nil {
			return err
		}
		p.log.Printf("archive %s/%s %s -> %s", req.owner, req.repo, req.ref, sha)
		w.Header().Set("Content-Type", "application/x-gzip")
		w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=%s-%s.tar.gz", req.repo, sha))
		w.Header().Set("ETag", fmt.Sprintf("%q", sha))
		if err := p.backend.writeArchive(ctx, w, dir, req.repo, sha); err != nil {
			p.log.Printf("archive %s/%s %s: %v", req.owner, req.repo, sha, err)
			// The status is already sent, so cut the response short for the client to notice.
			panic(http.ErrAbortHandler)
		}
		return nil
	}
	return fmt.Errorf("unknown request kind %d", req.kind)
}

func writeGitHubError(w http.ResponseWriter, status int, message string) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(map[string]string{"message": "ghshim: " + message})
}

type upstreamStatusError struct{ status string }

func (e *upstreamStatusError) Error() string { return "upstream proxy answered " + e.status }

func dialThroughUpstream(upstream *url.URL, target string) (net.Conn, error) {
	conn, err := net.DialTimeout("tcp", upstream.Host, 30*time.Second)
	if err != nil {
		return nil, err
	}
	var req strings.Builder
	fmt.Fprintf(&req, "CONNECT %s HTTP/1.1\r\nHost: %s\r\n", target, target)
	if user := upstream.User; user != nil {
		password, _ := user.Password()
		auth := base64.StdEncoding.EncodeToString([]byte(user.Username() + ":" + password))
		fmt.Fprintf(&req, "Proxy-Authorization: Basic %s\r\n", auth)
	}
	req.WriteString("\r\n")
	if _, err := io.WriteString(conn, req.String()); err != nil {
		conn.Close()
		return nil, err
	}
	reader := bufio.NewReader(conn)
	resp, err := http.ReadResponse(reader, &http.Request{Method: http.MethodConnect})
	if err != nil {
		conn.Close()
		return nil, err
	}
	if resp.StatusCode != http.StatusOK {
		conn.Close()
		return nil, &upstreamStatusError{status: resp.Status}
	}
	return withBuffered(conn, reader), nil
}

// withBuffered returns conn with the bytes that reader has already buffered put back in front.
func withBuffered(conn net.Conn, reader *bufio.Reader) net.Conn {
	n := reader.Buffered()
	if n == 0 {
		return conn
	}
	buffered, _ := reader.Peek(n)
	return &prefixedConn{Conn: conn, reader: io.MultiReader(bytes.NewReader(bytes.Clone(buffered)), conn)}
}

type prefixedConn struct {
	net.Conn
	reader io.Reader
}

func (c *prefixedConn) Read(b []byte) (int, error) { return c.reader.Read(b) }

func (c *prefixedConn) CloseWrite() error {
	if cw, ok := c.Conn.(interface{ CloseWrite() error }); ok {
		return cw.CloseWrite()
	}
	return nil
}

func copyAndCloseWrite(dst, src net.Conn) {
	io.Copy(dst, src)
	if cw, ok := dst.(interface{ CloseWrite() error }); ok {
		cw.CloseWrite()
	} else {
		dst.Close()
	}
}

// singleConnListener lets http.Server serve one connection, with keep-alive, and return once
// the connection closes.
type singleConnListener struct {
	conn     net.Conn
	accepted bool
	mu       sync.Mutex
	done     chan struct{}
	once     sync.Once
}

func newSingleConnListener(conn net.Conn) *singleConnListener {
	return &singleConnListener{conn: conn, done: make(chan struct{})}
}

func (l *singleConnListener) Accept() (net.Conn, error) {
	l.mu.Lock()
	if !l.accepted {
		l.accepted = true
		l.mu.Unlock()
		return l.conn, nil
	}
	l.mu.Unlock()
	<-l.done
	return nil, net.ErrClosed
}

func (l *singleConnListener) Close() error {
	l.once.Do(func() { close(l.done) })
	return nil
}

func (l *singleConnListener) Addr() net.Addr { return l.conn.LocalAddr() }

func (l *singleConnListener) connState(_ net.Conn, state http.ConnState) {
	if state == http.StateClosed || state == http.StateHijacked {
		l.Close()
	}
}
