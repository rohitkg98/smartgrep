from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Protocol, TypeAlias

EntityId: TypeAlias = int


class Validatable(Protocol):
    def validate(self) -> list[str]: ...


class Entity(ABC):
    id: EntityId

    def __init__(self, id: EntityId) -> None:
        self.id = id

    @abstractmethod
    def to_dict(self) -> dict[str, object]:
        raise NotImplementedError


class ValidationError(Exception):
    def __init__(self, errors: list[str]):
        super().__init__("; ".join(errors))
        self.errors = errors
