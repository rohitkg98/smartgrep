import { Validatable } from './models';

export function validateAll(items: Validatable[]): boolean {
    for (const item of items) {
        try {
            item.validate();
        } catch {
            return false;
        }
    }
    return true;
}

export const formatId = (prefix: string, id: number): string => {
    return `${prefix}-${id}`;
};

export const MAX_BATCH_SIZE = 100;
export const DEFAULT_PAGE_SIZE = 20;

export namespace Pagination {
    export interface PageRequest {
        page: number;
        size: number;
    }

    export interface PageResponse<T> {
        items: T[];
        total: number;
        page: number;
    }

    export function paginate<T>(items: T[], request: PageRequest): PageResponse<T> {
        const start = request.page * request.size;
        const end = start + request.size;
        return {
            items: items.slice(start, end),
            total: items.length,
            page: request.page,
        };
    }
}

export type Predicate<T> = (item: T) => boolean;
