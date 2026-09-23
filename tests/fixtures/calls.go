// Call-graph fixture for the Go parser.

package calls

import (
	"fmt"
	str "strings"
	yaml "gopkg.in/yaml.v3"
	"github.com/acme/app/v2"
)

type Store struct {
	items []string
	inner *Store
}

func NewStore() *Store {
	s := &Store{items: make([]string, 0)}
	s.Add("x")
	return s
}

func (s *Store) Add(item string) {
	s.items = append(s.items, str.ToUpper(item))
	s.inner.Flush()
	fmt.Println(len(s.items))
	fmt.Println("again")
	helper(item)
	go func() {
		background()
	}()
	_ = app.Version()
	_, _ = yaml.Marshal(item)
	_ = int64(len(item))
}

func (s *Store) Flush() {}

func helper(s string) string {
	return fmt.Sprintf("%s", s)
}

func background() {}
