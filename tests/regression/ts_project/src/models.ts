export interface Validatable {
    validate(): void;
}

export interface Identifiable {
    getId(): number;
    displayId(): string;
}

export interface Repository<T> {
    findById(id: number): T | null;
    save(entity: T): number;
    delete(id: number): void;
    listAll(): T[];
}

export class User implements Validatable, Identifiable {
    private active: boolean;

    constructor(
        public readonly id: number,
        public name: string,
        public email: string,
    ) {
        this.active = true;
    }

    deactivate(): void {
        this.active = false;
    }

    isActive(): boolean {
        return this.active;
    }

    validate(): void {
        if (!this.name || this.name.length === 0) {
            throw new ValidationError('name cannot be empty');
        }
    }

    getId(): number {
        return this.id;
    }

    displayId(): string {
        return `user-${this.id}`;
    }

    toString(): string {
        return `User(${this.id}, ${this.name})`;
    }
}

export class Role {
    constructor(
        public name: string,
        public permissions: Permission[],
    ) {}
}

export enum Permission {
    Read = 'read',
    Write = 'write',
    Admin = 'admin',
}

export enum Status {
    Active,
    Inactive,
    Suspended,
}

export class ValidationError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'ValidationError';
    }
}

export type UserID = number;
export type RoleName = string;
