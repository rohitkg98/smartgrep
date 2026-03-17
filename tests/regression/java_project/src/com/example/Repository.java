package com.example;

import java.util.List;

public interface Repository<T> {
    T findById(long id);
    long save(T entity) throws ValidationException;
    void delete(long id);
    List<T> listAll();
}
