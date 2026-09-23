"""Sample module exercising the Python parser."""
from __future__ import annotations

import os
import collections.abc as cabc
from typing import Generic, NewType, Protocol, TypeAlias, TypeVar, Union
from dataclasses import dataclass, field
from . import helpers
from .base import BaseModel, Validator as V
from ..core.errors import *

if os.name == "nt":
    import ntpath as pathmod
else:
    import posixpath as pathmod

MAX_RETRIES = 3
DEFAULT_TIMEOUT: float = 30.0
_INTERNAL_LIMIT = 10
logger = None
__all__ = ["User", "Repository"]

T = TypeVar("T")
AccountId = NewType("AccountId", int)
UserId: TypeAlias = int
JsonValue = Union[str, int, None]
Lookup = CACHE["x"] if False else None
type Callback = Callable[[int], None]
type Pair[K, V] = tuple[K, V]


class Repository(Protocol[T]):
    """Structural interface."""

    def get(self, key: str) -> T | None: ...

    def put(self, key: str, value: T) -> None: ...


@dataclass(frozen=True)
class User(BaseModel):
    name: str
    age: int = 0
    tags: list[str] = field(default_factory=list)
    _secret: str = ""
    kind = "user"
    __slots__ = ()

    def __init__(self, name: str, age: int = 0) -> None:
        super().__init__()
        self._setup()
        self.name = name
        self.email: str | None = None
        if age > 0:
            self.age = age
        self._cache = {}

    @property
    def display_name(self) -> str:
        return self.name.title()

    @staticmethod
    def validate(value, *args, strict: bool = False, **kwargs) -> bool:
        return True

    @classmethod
    def create(cls, name: str) -> "User":
        return cls(name)

    async def save(self, *, force=False) -> None:
        def _inner():
            return 1

        await _inner()

    def _private_helper(self):
        pass

    def __repr__(self) -> str:
        return f"User({self.name})"


class Cache(Generic[T], cabc.Mapping, metaclass=type):
    class Entry:
        value: int

    def lookup(self, key: T, /, default=None):
        return default


class _Hidden:
    pass


def top_level(a, b: int, c=1, d: str = "x", *rest, **opts) -> dict[str, int]:
    def nested_helper():
        return helpers.normalize(a)

    os.path.join("a", "b")
    pathmod.sep.join([])
    cabc.Mapping.register(dict)
    items = [transform(x) for x in rest]
    fn = lambda v: convert(v)
    User.create("x")
    User("bob")
    Cache[int]()
    get_user(1)
    get_user(2)
    opts.items.append(1)
    make().finish()
    logger.info("x")
    return {}


async def fetch(url: str, timeout: float = DEFAULT_TIMEOUT) -> bytes:
    return b""


@app.route("/users/<id>")
def get_user(id):
    return None


def _private_fn():
    pass
