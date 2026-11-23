import os.path
from collections.abc import Mapping
import tomllib
from pydantic_settings.sources import EnvSettingsSource
import httpx
from zope.interface import Interface


def fetch(cfg: Mapping[str, str]) -> str:
    return os.path.join(cfg.get("root", "."), str(EnvSettingsSource), str(httpx), str(Interface), str(tomllib))
