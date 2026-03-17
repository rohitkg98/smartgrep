package com.example;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.ArrayList;

public class UserService implements Repository<User> {
    private final Map<Long, User> users = new HashMap<>();
    private long nextId = 1;

    public long createUser(String name, String email) throws ValidationException {
        User user = new User(nextId++, name, email);
        user.validate();
        users.put(user.getId(), user);
        return user.getId();
    }

    public void deactivateUser(long id) {
        User user = users.get(id);
        if (user != null) {
            user.deactivate();
        }
    }

    @Override
    public User findById(long id) {
        return users.get(id);
    }

    @Override
    public long save(User entity) throws ValidationException {
        entity.validate();
        users.put(entity.getId(), entity);
        return entity.getId();
    }

    @Override
    public void delete(long id) {
        users.remove(id);
    }

    @Override
    public List<User> listAll() {
        return new ArrayList<>(users.values());
    }

    private long generateId() {
        return nextId++;
    }
}
