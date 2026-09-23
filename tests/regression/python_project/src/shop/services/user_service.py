from __future__ import annotations

import logging
from typing import Generic, Iterable, TypeVar

from ..models.base import Entity, EntityId
from ..models.user import User, Role

log = logging.getLogger(__name__)
DEFAULT_PAGE_SIZE = 20

E = TypeVar("E", bound=Entity)


class Repository(Generic[E]):
    def __init__(self) -> None:
        self._items: dict[EntityId, E] = {}

    def add(self, item: E) -> None:
        self._items[item.id] = item

    def get(self, id: EntityId) -> E | None:
        return self._items.get(id)


class UserService:
    def __init__(self, repo: Repository[User]):
        self.repo = repo

    async def register(self, name: str, email: str, *, admin: bool = False) -> User:
        user = User(id=len(self.repo._items) + 1, name=name, email=email,
                    role=Role.ADMIN if admin else Role.MEMBER)
        user.ensure_valid()
        self.repo.add(user)
        log.info("registered %s", name)
        return user


def paginate(items: Iterable[User], page: int = 0, size: int = DEFAULT_PAGE_SIZE) -> list[User]:
    items = list(items)
    return items[page * size:(page + 1) * size]
