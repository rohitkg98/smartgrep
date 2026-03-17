package main

import "fmt"

// Repository is a generic data access interface.
type Repository interface {
	FindByID(id int64) (interface{}, error)
	Save(entity interface{}) (int64, error)
	Delete(id int64) error
	ListAll() []interface{}
}

// UserService manages users.
type UserService struct {
	users  map[int64]*User
	nextID int64
}

// NewUserService creates a new service.
func NewUserService() *UserService {
	return &UserService{users: make(map[int64]*User), nextID: 1}
}

// CreateUser adds a new user.
func (s *UserService) CreateUser(name, email string) (int64, error) {
	user := NewUser(s.nextID, name, email)
	if err := user.Validate(); err != nil {
		return 0, fmt.Errorf("validation failed: %w", err)
	}
	id := user.GetID()
	s.users[id] = user
	s.nextID++
	return id, nil
}

// DeactivateUser marks a user as inactive.
func (s *UserService) DeactivateUser(id int64) error {
	user, ok := s.users[id]
	if !ok {
		return fmt.Errorf("user %d not found", id)
	}
	user.Deactivate()
	return nil
}

// FindByID returns a user by ID.
func (s *UserService) FindByID(id int64) (*User, error) {
	user, ok := s.users[id]
	if !ok {
		return nil, fmt.Errorf("user %d not found", id)
	}
	return user, nil
}

func (s *UserService) generateID() int64 {
	id := s.nextID
	s.nextID++
	return id
}
