// Test fixture for Go parser tests.
// Contains a variety of Go constructs.

package sample

import (
	"fmt"
	"io"
)

// Config is a public struct with exported and unexported fields.
type Config struct {
	Name    string
	Values  []string
	timeout int
}

// Handler is a public interface.
type Handler interface {
	Handle(input string) error
	Name() string
}

// NewConfig is a top-level public function.
func NewConfig(name string, timeout int) *Config {
	return &Config{Name: name, timeout: timeout}
}

// helper is a private top-level function.
func helper(s string) string {
	return fmt.Sprintf("help: %s", s)
}

// GetName is a public method on Config.
func (c *Config) GetName() string {
	return c.Name
}

// addValue is a private method on Config.
func (c *Config) addValue(v string) {
	c.Values = append(c.Values, v)
}

// StatusCode is a type alias for int.
type StatusCode = int

// Mode is a type definition (not alias).
type Mode int

// Public and private constants, including an iota group.
const MaxSize = 1024

const (
	ModeRead Mode = iota
	ModeWrite
	ModeReadWrite
)

const internalVersion = "1.0.0"

// Writer is an unexported interface.
type writer interface {
	Write(data []byte) (int, error)
}

// Embed tests: struct with embedded interface.
type Server struct {
	io.Reader
	Host string
	Port int
}

// Serve is a method on Server.
func (s *Server) Serve() error {
	return nil
}

// Ensure fmt and io are used.
var _ = fmt.Sprintf
var _ io.Reader
