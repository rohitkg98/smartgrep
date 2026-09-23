from dataclasses import dataclass, field
from enum import Enum

from .base import Entity, Validatable, ValidationError

MAX_NAME_LENGTH = 64


class Role(str, Enum):
    ADMIN = "admin"
    MEMBER = "member"


@dataclass
class User(Entity, Validatable):
    name: str
    email: str
    role: Role = Role.MEMBER
    tags: list[str] = field(default_factory=list)
    _password_hash: str = ""

    def validate(self) -> list[str]:
        errors = []
        if len(self.name) > MAX_NAME_LENGTH:
            errors.append("name too long")
        return errors

    def to_dict(self) -> dict[str, object]:
        return {"id": self.id, "name": self.name, "role": self.role.value}

    @property
    def is_admin(self) -> bool:
        return self.role is Role.ADMIN

    def ensure_valid(self) -> None:
        if errs := self.validate():
            raise ValidationError(errs)
