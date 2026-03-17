package com.example;

import java.util.Objects;

public class User implements Validatable, Identifiable {
    private long id;
    private String name;
    private String email;
    private boolean active;

    public User(long id, String name, String email) {
        this.id = id;
        this.name = name;
        this.email = email;
        this.active = true;
    }

    public void deactivate() {
        this.active = false;
    }

    public boolean isActive() {
        return active;
    }

    @Override
    public void validate() throws ValidationException {
        if (name == null || name.isEmpty()) {
            throw new ValidationException("name cannot be empty");
        }
    }

    @Override
    public long getId() {
        return id;
    }

    @Override
    public String displayId() {
        return "user-" + id;
    }

    @Override
    public String toString() {
        return String.format("User(%d, %s)", id, name);
    }
}
