use std::fmt;

use crate::errors::AppError;

const MAX_NAME_LENGTH: usize = 100;

#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    active: bool,
}

#[derive(Debug, Clone)]
pub struct Role {
    pub name: String,
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Permission {
    Read,
    Write,
    Admin,
}

pub enum Status {
    Active,
    Inactive,
    Suspended(String),
}

pub trait Validatable {
    fn validate(&self) -> Result<(), AppError>;
}

pub trait Identifiable {
    fn id(&self) -> u64;
    fn display_id(&self) -> String;
}

impl User {
    pub fn new(id: u64, name: String, email: String) -> Self {
        User { id, name, email, active: true }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Validatable for User {
    fn validate(&self) -> Result<(), AppError> {
        if self.name.is_empty() {
            return Err(AppError::Validation("name cannot be empty".into()));
        }
        if self.name.len() > MAX_NAME_LENGTH {
            return Err(AppError::Validation("name too long".into()));
        }
        Ok(())
    }
}

impl Identifiable for User {
    fn id(&self) -> u64 {
        self.id
    }

    fn display_id(&self) -> String {
        format!("user-{}", self.id)
    }
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "User({}, {})", self.id, self.name)
    }
}
