// Sample TypeScript fixture for parser tests

import { EventEmitter } from 'events';
import * as path from 'path';

// --- Type aliases ---

export type Status = 'active' | 'inactive' | 'pending';

type UserId = string;

// --- Enums ---

export enum Color {
    Red = 'RED',
    Green = 'GREEN',
    Blue = 'BLUE',
}

const enum Direction {
    Up,
    Down,
    Left,
    Right,
}

// --- Interfaces ---

export interface Serializable {
    serialize(): string;
    deserialize(data: string): void;
}

interface Config {
    readonly host: string;
    port: number;
    debug?: boolean;
}

export interface Repository<T> extends Serializable {
    findById(id: string): T | null;
    save(entity: T): void;
    delete(id: string): boolean;
}

// --- Classes ---

@Injectable()
export class UserService extends EventEmitter implements Serializable {
    private id: string;
    public name: string;
    protected email: string;
    readonly createdAt: Date;

    constructor(id: string, name: string) {
        super();
        this.id = id;
        this.name = name;
    }

    serialize(): string {
        return JSON.stringify({ id: this.id, name: this.name });
    }

    deserialize(data: string): void {
        const parsed = JSON.parse(data);
        this.id = parsed.id;
        this.name = parsed.name;
    }

    @Log()
    public getName(): string {
        return this.name;
    }

    private validate(): boolean {
        return this.id.length > 0;
    }

    static create(name: string): UserService {
        return new UserService('auto', name);
    }
}

export abstract class BaseRepository<T> {
    abstract findById(id: string): T | null;
    abstract save(entity: T): void;

    protected log(message: string): void {
        console.log(message);
    }
}

class InternalHelper {
    run(): void {
        console.log('running');
    }
}

// --- Functions ---

export function greet(name: string): string {
    return `Hello, ${name}!`;
}

function helper(x: number, y: number): number {
    return x + y;
}

// --- Arrow functions ---

export const fetchUser = (id: string): Promise<UserService> => {
    return Promise.resolve(new UserService(id, 'test'));
};

const internalUtil = (value: string): boolean => {
    return value.length > 0;
};

// --- Const (non-function) ---

export const MAX_RETRIES = 3;

const DEFAULT_PORT = 8080;

// --- Namespace ---

export namespace Validation {
    export function isValid(input: string): boolean {
        return input.length > 0;
    }

    export interface Validator {
        validate(value: string): boolean;
    }
}

// --- Call graph ---

const registry = new Map<string, number>();
registry.set('boot', 1);

export class Dispatcher extends EventEmitter {
    private items: string[] = [];

    constructor() {
        super();
        this.setup();
    }

    private setup(): void {}

    run(xs: string[]): void {
        helper(1, 2);
        helper(3, 4);
        path.join('a', 'b');
        Math.max(1, 2);
        JSON.stringify(xs);
        this.items.push('a');
        console.log('run');
        const m = new Map<string, Array<number>>();
        const d = new Validation.Checker();
        xs.forEach((x) => this.process(x));
        identity<string>('x');
        build().finish();
        this.emit?.('done');
    }
}

export const runAll = (ds: Dispatcher[]): void => {
    ds.forEach((d) => d.run([]));
    greet('all');
    function inner() {
        nestedCall();
    }
};
