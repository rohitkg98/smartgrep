use std::collections::HashMap;

use crate::errors::AppError;
use crate::models::{User, Validatable, Identifiable};

pub struct UserService {
    users: HashMap<u64, User>,
    next_id: u64,
}

pub trait Repository<T> {
    fn find_by_id(&self, id: u64) -> Option<&T>;
    fn save(&mut self, entity: T) -> Result<u64, AppError>;
    fn delete(&mut self, id: u64) -> Result<(), AppError>;
    fn list_all(&self) -> Vec<&T>;
}

impl UserService {
    pub fn new() -> Self {
        UserService { users: HashMap::new(), next_id: 1 }
    }

    pub fn create_user(&mut self, name: String, email: String) -> Result<u64, AppError> {
        let user = User::new(self.next_id, name, email);
        user.validate()?;
        let id = user.id();
        self.users.insert(id, user);
        self.next_id += 1;
        Ok(id)
    }

    pub fn deactivate_user(&mut self, id: u64) -> Result<(), AppError> {
        match self.users.get_mut(&id) {
            Some(user) => {
                user.deactivate();
                Ok(())
            }
            None => Err(AppError::NotFound(format!("user {}", id))),
        }
    }

    fn generate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

impl Repository<User> for UserService {
    fn find_by_id(&self, id: u64) -> Option<&User> {
        self.users.get(&id)
    }

    fn save(&mut self, entity: User) -> Result<u64, AppError> {
        entity.validate()?;
        let id = entity.id();
        self.users.insert(id, entity);
        Ok(id)
    }

    fn delete(&mut self, id: u64) -> Result<(), AppError> {
        self.users.remove(&id)
            .map(|_| ())
            .ok_or_else(|| AppError::NotFound(format!("user {}", id)))
    }

    fn list_all(&self) -> Vec<&User> {
        self.users.values().collect()
    }
}
