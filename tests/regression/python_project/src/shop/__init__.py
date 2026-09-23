"""Tiny shop package used by smartgrep regression tests."""
from .models.base import Entity
from .models.user import User

__all__ = ["Entity", "User"]
VERSION = "0.1.0"
