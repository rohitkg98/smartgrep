import { User, Validatable, Repository, ValidationError } from '../models';

export class UserService implements Repository<User> {
    private users: Map<number, User> = new Map();
    private nextId: number = 1;

    createUser(name: string, email: string): number {
        const user = new User(this.nextId++, name, email);
        user.validate();
        this.users.set(user.getId(), user);
        return user.getId();
    }

    deactivateUser(id: number): void {
        const user = this.users.get(id);
        if (!user) {
            throw new Error(`user ${id} not found`);
        }
        user.deactivate();
    }

    findById(id: number): User | null {
        return this.users.get(id) ?? null;
    }

    save(entity: User): number {
        entity.validate();
        this.users.set(entity.getId(), entity);
        return entity.getId();
    }

    delete(id: number): void {
        this.users.delete(id);
    }

    listAll(): User[] {
        return Array.from(this.users.values());
    }

    private generateId(): number {
        return this.nextId++;
    }
}

export abstract class BaseService<T extends Validatable> {
    abstract findById(id: number): T | null;
    abstract save(entity: T): number;

    protected log(message: string): void {
        console.log(`[${this.constructor.name}] ${message}`);
    }
}

export const createUserService = (): UserService => {
    return new UserService();
};
