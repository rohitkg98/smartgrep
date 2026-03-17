package main

import "fmt"

// AppError is the base error type.
type AppError struct {
	Code    string
	Message string
}

func (e *AppError) Error() string {
	return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

// NewValidationError creates a validation error.
func NewValidationError(msg string) *AppError {
	return &AppError{Code: "VALIDATION", Message: msg}
}

// NewNotFoundError creates a not-found error.
func NewNotFoundError(msg string) *AppError {
	return &AppError{Code: "NOT_FOUND", Message: msg}
}

// WrapError adds context to an existing error.
func WrapError(msg string, err error) error {
	return fmt.Errorf("%s: %w", msg, err)
}
