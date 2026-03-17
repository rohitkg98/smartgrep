package main

import "fmt"

// User represents a system user.
type User struct {
	ID     int64
	Name   string
	Email  string
	active bool
}

// Role represents an authorization role.
type Role struct {
	Name        string
	Permissions []Permission
}

// Permission is a typed string for access control.
type Permission string

const (
	PermRead  Permission = "read"
	PermWrite Permission = "write"
	PermAdmin Permission = "admin"
)

// Status tracks the lifecycle state.
type Status int

const (
	StatusActive Status = iota
	StatusInactive
	StatusSuspended
)

// Validatable is anything that can self-validate.
type Validatable interface {
	Validate() error
}

// Identifiable has a unique ID.
type Identifiable interface {
	GetID() int64
	DisplayID() string
}

// NewUser creates a new active user.
func NewUser(id int64, name, email string) *User {
	return &User{ID: id, Name: name, Email: email, active: true}
}

// Deactivate marks the user inactive.
func (u *User) Deactivate() {
	u.active = false
}

// IsActive checks if the user is active.
func (u *User) IsActive() bool {
	return u.active
}

// Validate checks user fields.
func (u *User) Validate() error {
	if u.Name == "" {
		return fmt.Errorf("name cannot be empty")
	}
	return nil
}

// GetID returns the user ID.
func (u *User) GetID() int64 {
	return u.ID
}

// DisplayID returns a formatted ID string.
func (u *User) DisplayID() string {
	return fmt.Sprintf("user-%d", u.ID)
}
